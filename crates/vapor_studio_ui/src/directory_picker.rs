use super::{
    StudioApp,
    components::design::{ACCENT_FILL, StatusTone, status_text},
};
use eframe::egui;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
pub(super) enum DirectoryPickerPurpose {
    OpenWorkspace,
    WorkspaceParent,
}

pub(super) enum DirectoryPickerResult {
    Cancelled,
    Selected {
        purpose: DirectoryPickerPurpose,
        path: PathBuf,
    },
}

pub(super) struct DirectoryPicker {
    purpose: DirectoryPickerPurpose,
    path: PathBuf,
    path_draft: String,
    folder_filter: String,
    directories: Vec<PathBuf>,
    overflow: bool,
    error: Option<String>,
    open: bool,
}

impl DirectoryPicker {
    pub(super) fn new(purpose: DirectoryPickerPurpose, initial: &Path) -> Self {
        let starting_path = fs::canonicalize(initial)
            .ok()
            .filter(|path| path.is_dir())
            .or_else(|| {
                initial
                    .parent()
                    .and_then(|parent| fs::canonicalize(parent).ok())
                    .filter(|path| path.is_dir())
            })
            .or_else(home_directory)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let mut picker = Self {
            purpose,
            path: PathBuf::new(),
            path_draft: String::new(),
            folder_filter: String::new(),
            directories: Vec::new(),
            overflow: false,
            error: None,
            open: true,
        };
        picker.navigate(&starting_path);
        picker
    }

    fn navigate(&mut self, requested: &Path) {
        match fs::canonicalize(requested) {
            Ok(path) if path.is_dir() => {
                self.path = path;
                self.path_draft = self.path.display().to_string();
                self.error = None;
            }
            Ok(path) => {
                self.error = Some(format!("{} is not a folder.", path.display()));
                return;
            }
            Err(error) => {
                self.error = Some(format!("Could not open {}: {error}", requested.display()));
                return;
            }
        }

        let mut directories = Vec::new();
        let mut overflow = false;
        match fs::read_dir(&self.path) {
            Ok(entries) => {
                for entry in entries {
                    let Ok(entry) = entry else { continue };
                    let Ok(kind) = entry.file_type() else {
                        continue;
                    };
                    // Follow a directory symlink only after the user navigates into it.
                    // Navigation canonicalizes the target, so parent traversal stays bounded.
                    let is_directory = kind.is_dir()
                        || (kind.is_symlink()
                            && fs::metadata(entry.path()).is_ok_and(|metadata| metadata.is_dir()));
                    if !is_directory {
                        continue;
                    }
                    if directories.len() == 500 {
                        overflow = true;
                        break;
                    }
                    directories.push(entry.path());
                }
                directories.sort_by_key(|path| {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                });
            }
            Err(error) => {
                self.error = Some(format!("Could not list {}: {error}", self.path.display()));
            }
        }
        self.directories = directories;
        self.overflow = overflow;
    }

    pub(super) fn show(&mut self, context: &egui::Context) -> Option<DirectoryPickerResult> {
        let title = match self.purpose {
            DirectoryPickerPurpose::OpenWorkspace => "Open a project folder",
            DirectoryPickerPurpose::WorkspaceParent => "Choose where to create a project folder",
        };
        let mut window_open = self.open;
        let mut result = None;
        let viewport = context.content_rect().size();
        let picker_size = egui::vec2((viewport.x - 24.0).max(1.0), (viewport.y - 24.0).max(1.0));
        let folders_height = (viewport.y - 300.0).clamp(40.0, 300.0);
        egui::Window::new(title)
            .id(egui::Id::new("studio-directory-picker"))
            .open(&mut window_open)
            .collapsible(false)
            .resizable(true)
            .default_width((viewport.x - 24.0).clamp(1.0, 620.0))
            .max_size(picker_size)
            .show(context, |ui| {
                ui.label(match self.purpose {
                    DirectoryPickerPurpose::OpenWorkspace => {
                        "Choose a folder containing a Vapor project."
                    }
                    DirectoryPickerPurpose::WorkspaceParent => {
                        "Studio will create a new project folder inside this location."
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    let up = self.path.parent().map(Path::to_path_buf);
                    if ui
                        .add_enabled(up.is_some(), egui::Button::new("↑ Up"))
                        .clicked()
                        && let Some(up) = up
                    {
                        self.navigate(&up);
                    }
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.path_draft)
                            .desired_width(ui.available_width().clamp(1.0, 480.0))
                            .hint_text("Folder path"),
                    );
                    let go = ui.button("Go").clicked()
                        || (response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                    if go {
                        let requested = PathBuf::from(self.path_draft.trim());
                        self.navigate(&requested);
                    }
                });

                if let Some(error) = &self.error {
                    status_text(ui, error, StatusTone::Error);
                }
                let recognized_workspace = self.path.join("Workspace.vapor.toml").is_file();
                if matches!(self.purpose, DirectoryPickerPurpose::OpenWorkspace) {
                    if recognized_workspace {
                        status_text(ui, "Vapor project found here", StatusTone::Positive);
                    } else {
                        ui.weak("This folder does not contain a Vapor project definition.");
                    }
                }
                ui.separator();
                ui.label(egui::RichText::new(self.path.display().to_string()).strong());
                ui.add(
                    egui::TextEdit::singleline(&mut self.folder_filter)
                        .hint_text("Filter folders in this location"),
                );
                egui::ScrollArea::vertical()
                    .max_height(folders_height)
                    .show(ui, |ui| {
                        let filter = self.folder_filter.trim().to_lowercase();
                        let mut visible_directories = 0;
                        let mut navigate_to = None;
                        for directory in &self.directories {
                            let name = directory
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy();
                            if !filter.is_empty() && !name.to_lowercase().contains(&filter) {
                                continue;
                            }
                            visible_directories += 1;
                            if ui.selectable_label(false, format!("›  {name}")).clicked() {
                                navigate_to = Some(directory.clone());
                            }
                        }
                        if visible_directories == 0 {
                            ui.weak(if filter.is_empty() {
                                "No subfolders here."
                            } else {
                                "No folders match this filter."
                            });
                        } else {
                            ui.small(format!("{visible_directories} folders shown"));
                        }
                        if self.overflow {
                            ui.weak("Showing the first 500 folders. Type a path above to narrow the view.");
                        }
                        if let Some(directory) = navigate_to {
                            self.navigate(&directory);
                        }
                    });

                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Cancel").clicked() {
                        result = Some(DirectoryPickerResult::Cancelled);
                    }
                    let can_select = match self.purpose {
                        DirectoryPickerPurpose::OpenWorkspace => {
                            self.error.is_none() && recognized_workspace
                        }
                        DirectoryPickerPurpose::WorkspaceParent => self.error.is_none(),
                    };
                    let label = match self.purpose {
                        DirectoryPickerPurpose::OpenWorkspace => "Open this project folder",
                        DirectoryPickerPurpose::WorkspaceParent => "Choose this location",
                    };
                    if ui
                        .add_enabled(can_select, egui::Button::new(label).fill(ACCENT_FILL))
                        .clicked()
                    {
                        result = Some(DirectoryPickerResult::Selected {
                            purpose: self.purpose,
                            path: self.path.clone(),
                        });
                    }
                });
            });
        self.open = window_open;
        result.or_else(|| (!self.open).then_some(DirectoryPickerResult::Cancelled))
    }
}

fn home_directory() -> Option<PathBuf> {
    [std::env::var_os("USERPROFILE"), std::env::var_os("HOME")]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .find(|path| path.is_dir())
}

impl StudioApp {
    pub(super) fn open_workspace_picker(&mut self) {
        let draft = PathBuf::from(self.workspace.root_draft.trim());
        let initial =
            if self.workspace.root.trim().is_empty() && draft == self.workspace.fallback_root {
                PathBuf::new()
            } else {
                draft
            };
        self.directory_picker = Some(DirectoryPicker::new(
            DirectoryPickerPurpose::OpenWorkspace,
            &initial,
        ));
    }

    pub(super) fn open_workspace_parent_picker(&mut self) {
        self.directory_picker = Some(DirectoryPicker::new(
            DirectoryPickerPurpose::WorkspaceParent,
            Path::new(self.workspace.starter_path.trim()),
        ));
    }
}
