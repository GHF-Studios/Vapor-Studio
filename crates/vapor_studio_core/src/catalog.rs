use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use vapor_shell::{
    content,
    discovery::{EnvironmentPaths, InstallationPaths},
};
use crate::ContentIdentity;

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
    /// Content tree fingerprint recorded by Vapor for cache/install sources.
    pub fingerprint: Option<String>,
    /// Original source Vapor recorded when this installed copy was created.
    pub install_source: Option<String>,
}

/// A same-version identity whose locally observed metadata disagrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogConflict {
    pub field: String,
    pub first_value: String,
    pub first_location: PathBuf,
    pub conflicting_value: String,
    pub conflicting_location: PathBuf,
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
    /// Conflicts make this identity unsafe to select or publish until resolved.
    pub conflicts: Vec<CatalogConflict>,
}

impl CatalogContent {
    pub fn identity(&self) -> ContentIdentity {
        ContentIdentity {
            id: self.id.clone(),
            version: self.version.clone(),
        }
    }

    pub fn is_unambiguous(&self) -> bool {
        self.conflicts.is_empty()
    }
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
            installation.clone(),
            workspace_root,
        ) && let Ok(source_catalog) = content::discover(&paths)
        {
            for artifact in source_catalog.artifacts() {
                merge_entry(
                    &mut entries,
                    CatalogContent {
                        id: artifact.id().to_owned(),
                        version: artifact.version().map(ToOwned::to_owned),
                        kind: artifact.kind().to_string(),
                        published_file_id: artifact
                            .workshop()
                            .published_file_id()
                            .map(ToOwned::to_owned),
                        dependencies: artifact
                            .dependencies()
                            .iter()
                            .map(dependency_description)
                            .collect(),
                        locations: vec![CatalogLocation {
                            kind: CatalogLocationKind::Workspace,
                            root: artifact.root().to_path_buf(),
                            enabled: true,
                            fingerprint: None,
                            install_source: None,
                        }],
                        conflicts: Vec::new(),
                    },
                );
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
                dependencies: artifact
                    .dependencies()
                    .iter()
                    .map(dependency_description)
                    .collect(),
                locations: vec![CatalogLocation {
                    kind: CatalogLocationKind::Cache,
                    root: artifact.installed_root().to_path_buf(),
                    enabled: true,
                    fingerprint: Some(format!(
                        "{}:{}",
                        artifact.fingerprint().algorithm(),
                        artifact.fingerprint().hash()
                    )),
                    install_source: None,
                }],
                conflicts: Vec::new(),
            },
        );
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
                    dependencies: artifact
                        .dependencies()
                        .iter()
                        .map(dependency_description)
                        .collect(),
                    locations: vec![CatalogLocation {
                        kind: CatalogLocationKind::Installed,
                        root: artifact.installed_root().to_path_buf(),
                        enabled: artifact.enabled(),
                        fingerprint: Some(format!(
                            "{}:{}",
                            artifact.fingerprint().algorithm(),
                            artifact.fingerprint().hash()
                        )),
                        install_source: Some(artifact.source().to_owned()),
                    }],
                    conflicts: Vec::new(),
                },
            );
        }
    }

    Ok(entries.into_values().collect())
}

fn dependency_description(dependency: &content::ContentReference) -> String {
    let version = dependency
        .version()
        .map(|version| format!(" @ {version}"))
        .unwrap_or_default();
    let optional = if dependency.optional() {
        " · optional"
    } else {
        ""
    };
    let workshop = dependency
        .workshop_id()
        .map(|id| format!(" · Workshop {id}"))
        .unwrap_or_default();
    let reason = dependency
        .reason()
        .map(|reason| format!(" · {reason}"))
        .unwrap_or_default();
    format!(
        "{} · {}{}{}{}{}",
        dependency.relationship(),
        dependency.id(),
        version,
        optional,
        workshop,
        reason
    )
}

fn merge_entry(
    entries: &mut BTreeMap<ContentIdentity, CatalogContent>,
    mut incoming: CatalogContent,
) {
    let key = incoming.identity();
    if let Some(existing) = entries.get_mut(&key) {
        let first_location = existing
            .locations
            .first()
            .map(|location| location.root.clone())
            .unwrap_or_default();
        let incoming_location = incoming
            .locations
            .first()
            .map(|location| location.root.clone())
            .unwrap_or_default();
        let mut conflicts = Vec::new();
        if existing.kind != incoming.kind {
            conflicts.push(("kind", existing.kind.clone(), incoming.kind.clone()));
        }
        if let (Some(first), Some(next)) = (&existing.published_file_id, &incoming.published_file_id)
            && first != next
        {
            conflicts.push(("PublishedFileId", first.clone(), next.clone()));
        }
        let mut first_dependencies = existing.dependencies.clone();
        let mut next_dependencies = incoming.dependencies.clone();
        first_dependencies.sort();
        next_dependencies.sort();
        if first_dependencies != next_dependencies {
            conflicts.push((
                "dependencies",
                first_dependencies.join(", "),
                next_dependencies.join(", "),
            ));
        }
        let first_fingerprint = existing
            .locations
            .iter()
            .find_map(|location| location.fingerprint.as_ref());
        let incoming_fingerprint = incoming
            .locations
            .iter()
            .find_map(|location| location.fingerprint.as_ref());
        if let (Some(first), Some(next)) = (first_fingerprint, incoming_fingerprint)
            && first != next
        {
            conflicts.push(("content fingerprint", first.clone(), next.clone()));
        }
        existing
            .conflicts
            .extend(conflicts.into_iter().map(
                |(field, first_value, conflicting_value)| CatalogConflict {
                    field: field.to_owned(),
                    first_value,
                    first_location: first_location.clone(),
                    conflicting_value,
                    conflicting_location: incoming_location.clone(),
                },
            ));
        existing.conflicts.append(&mut incoming.conflicts);
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
}
