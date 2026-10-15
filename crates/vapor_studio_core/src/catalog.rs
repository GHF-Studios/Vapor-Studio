use std::{collections::BTreeMap, path::{Path, PathBuf}};
use vapor_shell::{content, discovery::{EnvironmentPaths, InstallationPaths}};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogLocationKind {
    Workspace,
    Cache,
    Installed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogLocation {
    pub kind: CatalogLocationKind,
    pub root: PathBuf,
    pub enabled: bool,
}

/// One exact content version merged across all locally available sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogContent {
    pub id: String,
    pub version: Option<String>,
    pub kind: String,
    pub published_file_id: Option<String>,
    pub dependencies: Vec<String>,
    pub locations: Vec<CatalogLocation>,
}

/// Merge workspace-authored, cached, and installed content by exact identity.
/// A missing or invalid workspace does not prevent cache and install discovery
/// when the caller passes no workspace root.
pub fn discover_content_catalog(
    application_executable: &Path,
    workspace_root: Option<&Path>,
) -> Result<Vec<CatalogContent>, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let mut entries = BTreeMap::new();

    if let Some(workspace_root) = workspace_root {
        // Workspace source errors are surfaced through Studio's authored-source
        // discovery result. They must not hide a usable cache/install catalog.
        if let Ok(paths) = EnvironmentPaths::from_installation_and_source_path(
            installation.clone(), workspace_root,
        ) && let Ok(source_catalog) = content::discover(&paths) {
            for artifact in source_catalog.artifacts() {
                merge_entry(
                    &mut entries,
                    CatalogContent {
                        id: artifact.id().to_owned(),
                        version: artifact.version().map(ToOwned::to_owned),
                        kind: artifact.kind().to_string(),
                        published_file_id: artifact.workshop().published_file_id().map(ToOwned::to_owned),
                        dependencies: artifact.dependencies().iter().map(|dependency| {
                            format!("{} · {}", dependency.relationship(), dependency.id())
                        }).collect(),
                        locations: vec![CatalogLocation {
                            kind: CatalogLocationKind::Workspace,
                            root: artifact.root().to_path_buf(),
                            enabled: true,
                        }],
                    },
                )?;
            }
        }
    }

    for selector in content::cached_index(&installation)? {
        let artifact = content::cached_artifact(&installation, &selector)?;
        merge_entry(
            &mut entries,
            CatalogContent {
                id: artifact.artifact_id().to_owned(),
                version: artifact.version().map(ToOwned::to_owned),
                kind: artifact.kind().to_owned(),
                published_file_id: artifact.workshop_id().map(ToOwned::to_owned),
                dependencies: artifact.dependencies().iter().map(|dependency| {
                    format!("{} · {}", dependency.relationship(), dependency.id())
                }).collect(),
                locations: vec![CatalogLocation {
                    kind: CatalogLocationKind::Cache,
                    root: artifact.installed_root().to_path_buf(),
                    enabled: true,
                }],
            },
        )?;
    }

    for selector in content::installed_index(&installation)? {
        let artifacts = content::installed_artifacts(&installation, &[selector])?;
        for artifact in artifacts {
            merge_entry(
                &mut entries,
                CatalogContent {
                    id: artifact.artifact_id().to_owned(),
                    version: artifact.version().map(ToOwned::to_owned),
                    kind: artifact.kind().to_owned(),
                    published_file_id: artifact.workshop_id().map(ToOwned::to_owned),
                    dependencies: artifact.dependencies().iter().map(|dependency| {
                        format!("{} · {}", dependency.relationship(), dependency.id())
                    }).collect(),
                    locations: vec![CatalogLocation {
                        kind: CatalogLocationKind::Installed,
                        root: artifact.installed_root().to_path_buf(),
                        enabled: artifact.enabled(),
                    }],
                },
            )?;
        }
    }

    Ok(entries.into_values().collect())
}

fn merge_entry(
    entries: &mut BTreeMap<(String, Option<String>), CatalogContent>,
    mut incoming: CatalogContent,
) -> Result<(), String> {
    let key = (incoming.id.clone(), incoming.version.clone());
    if let Some(existing) = entries.get_mut(&key) {
        if existing.kind != incoming.kind {
            return Err(format!(
                "content {}{} has conflicting kinds '{}' and '{}' across local sources",
                key.0,
                key.1.as_ref().map(|version| format!("@{version}")).unwrap_or_default(),
                existing.kind,
                incoming.kind,
            ));
        }
        if existing.published_file_id.is_none() {
            existing.published_file_id = incoming.published_file_id.take();
        }
        if existing.dependencies.is_empty() {
            existing.dependencies = std::mem::take(&mut incoming.dependencies);
        }
        for location in incoming.locations {
            if !existing.locations.contains(&location) {
                existing.locations.push(location);
            }
        }
    } else {
        entries.insert(key, incoming);
    }
    Ok(())
}
