//! Bounded and guarded source-file access for the Workspace editor.

use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn discover_workspace_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let root = fs::canonicalize(root).map_err(|error| format!("{}: {error}", root.display()))?;
    let mut pending = vec![root.clone()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(
                name.as_ref(),
                ".git" | ".hg" | ".svn" | ".cache" | "target" | "node_modules" | "vendor" | "output"
            ) {
                continue;
            }
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(&root)
                    .map_err(|error| error.to_string())?
                    .to_path_buf();
                files.push(relative);
                if files.len() > 10_000 {
                    return Err(
                        "workspace contains more than 10,000 files; narrow the source root"
                            .to_owned(),
                    );
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

fn safe_workspace_file(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|error| format!("{}: {error}", root.display()))?;
    let path = fs::canonicalize(root.join(relative))
        .map_err(|error| format!("{}: {error}", relative.display()))?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err("selected path is not a regular file inside the workspace".to_owned());
    }
    Ok(path)
}

pub(super) fn read_workspace_file(root: &Path, relative: &Path) -> Result<String, String> {
    let path = safe_workspace_file(root, relative)?;
    let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
    if metadata.len() > 4 * 1024 * 1024 {
        return Err(
            "files larger than 4 MiB cannot be opened in the Studio text editor".to_owned(),
        );
    }
    fs::read_to_string(&path).map_err(|error| {
        format!(
            "{} is not a readable UTF-8 text file: {error}",
            relative.display()
        )
    })
}

pub(super) fn write_workspace_file(
    root: &Path,
    relative: &Path,
    expected_contents: &str,
    contents: &str,
) -> Result<(), String> {
    let path = safe_workspace_file(root, relative)?;
    let _lock = WorkspaceFileEditLock::acquire(workspace_edit_lock_path(&path))?;
    let metadata =
        fs::metadata(&path).map_err(|error| format!("{}: {error}", relative.display()))?;
    let current =
        fs::read_to_string(&path).map_err(|error| format!("{}: {error}", relative.display()))?;
    if current != expected_contents {
        return Err(format!(
            "{} changed on disk after it was opened; reload it before saving to avoid overwriting those edits",
            relative.display()
        ));
    }

    let temporary = sibling_edit_path(&path, "tmp");
    let backup = sibling_edit_path(&path, "backup");
    let write_result = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("could not prepare {}: {error}", relative.display()))?;
        use std::io::Write;
        file.write_all(contents.as_bytes())
            .map_err(|error| format!("could not write {}: {error}", relative.display()))?;
        fs::set_permissions(&temporary, metadata.permissions()).map_err(|error| {
            format!(
                "could not preserve permissions for {}: {error}",
                relative.display()
            )
        })?;
        file.sync_all()
            .map_err(|error| format!("could not sync {}: {error}", relative.display()))?;
        let latest = fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", relative.display()))?;
        if latest != expected_contents {
            return Err(format!(
                "{} changed on disk while the save was being prepared; reload before saving",
                relative.display()
            ));
        }
        fs::rename(&path, &backup).map_err(|error| {
            format!(
                "could not preserve original {}: {error}",
                relative.display()
            )
        })?;
        if let Err(error) = fs::rename(&temporary, &path) {
            return Err(match fs::rename(&backup, &path) {
                Ok(()) => format!(
                    "could not replace {}: {error}; original restored",
                    relative.display()
                ),
                Err(restore) => format!(
                    "could not replace {}: {error}; restoring original also failed: {restore}",
                    relative.display()
                ),
            });
        }
        let _ = fs::remove_file(&backup);
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

fn workspace_edit_lock_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("workspace-file"))
        .to_string_lossy();
    path.with_file_name(format!(".{name}.studio-edit.lock"))
}

fn sibling_edit_path(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("workspace-file"))
        .to_string_lossy();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.with_file_name(format!(
        ".{name}.{}.{}.studio-{suffix}",
        std::process::id(),
        nonce
    ))
}

struct WorkspaceFileEditLock {
    path: PathBuf,
    file: Option<fs::File>,
}

impl WorkspaceFileEditLock {
    fn acquire(path: PathBuf) -> Result<Self, String> {
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                format!(
                    "cannot acquire workspace file edit lock '{}': {error}",
                    path.display()
                )
            })?;
        Ok(Self {
            path,
            file: Some(file),
        })
    }
}

impl Drop for WorkspaceFileEditLock {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

pub(super) fn create_new_workspace_file(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(
            "file path must be relative and cannot contain '.', '..', or root components"
                .to_owned(),
        );
    }
    let root = fs::canonicalize(root).map_err(|error| format!("{}: {error}", root.display()))?;
    let requested = root.join(relative);
    let relative_parent = relative
        .parent()
        .ok_or_else(|| "file path has no parent directory".to_owned())?;
    let mut parent = root.clone();
    let mut created_directories = Vec::new();
    for component in relative_parent.components() {
        let Component::Normal(name) = component else {
            return Err("file parent path must stay inside the workspace".to_owned());
        };
        parent.push(name);
        match fs::symlink_metadata(&parent) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                cleanup_created_directories(&created_directories);
                return Err(format!(
                    "file parent must use regular workspace directories: {}",
                    parent.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Err(error) = fs::create_dir(&parent) {
                    cleanup_created_directories(&created_directories);
                    return Err(format!(
                        "could not create folder {}: {error}",
                        parent.display()
                    ));
                }
                created_directories.push(parent.clone());
            }
            Err(error) => {
                cleanup_created_directories(&created_directories);
                return Err(format!("{}: {error}", parent.display()));
            }
        }
        let canonical_parent = match fs::canonicalize(&parent) {
            Ok(path) => path,
            Err(error) => {
                cleanup_created_directories(&created_directories);
                return Err(format!("{}: {error}", parent.display()));
            }
        };
        if !canonical_parent.starts_with(&root) {
            cleanup_created_directories(&created_directories);
            return Err("file path must stay inside the workspace".to_owned());
        }
        parent = canonical_parent;
    }
    let Some(name) = requested.file_name() else {
        cleanup_created_directories(&created_directories);
        return Err("file path must include a filename".to_owned());
    };
    let target = parent.join(name);
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|error| {
            cleanup_created_directories(&created_directories);
            format!(
                "could not create {} without overwriting: {error}",
                relative.display()
            )
        })?;
    drop(file);
    Ok(relative.to_path_buf())
}

fn cleanup_created_directories(directories: &[PathBuf]) {
    for directory in directories.iter().rev() {
        let _ = fs::remove_dir(directory);
    }
}
