use std::path::{Path, PathBuf};
use vapor_shell::{
    content,
    discovery::{EnvironmentPaths, InstallationPaths},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationCheck {
    pub ok: bool,
    pub detail: Vec<String>,
    pub artifacts: Vec<InstalledArtifact>,
    pub graph: Option<crate::ResolvedCompositionGraph>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledArtifact {
    pub id: String,
    pub version: Option<String>,
    pub root: PathBuf,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkshopAcquisition {
    pub id: String,
    pub version: Option<String>,
    pub kind: String,
    pub cache_root: PathBuf,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledComposition {
    pub graph: crate::ResolvedCompositionGraph,
    pub artifacts: Vec<InstalledArtifact>,
}

/// Provenance selected for one node in a read-only composition preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionPreviewSource {
    Workspace,
    Cache,
    Installed,
    DownloadRequired,
}

/// One dependency edge shown before installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionPreviewDependency {
    pub id: String,
    pub relationship: String,
    pub version: Option<String>,
    pub optional: bool,
}

/// One exact node selected by Shell's pre-install resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionPreviewNode {
    pub id: String,
    pub version: Option<String>,
    pub kind: String,
    pub published_file_id: Option<String>,
    pub source: CompositionPreviewSource,
    pub already_installed: bool,
    pub dependencies: Vec<CompositionPreviewDependency>,
}

/// Read-only dependency resolution and any blockers that require user action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionPreview {
    pub selector: String,
    pub nodes: Vec<CompositionPreviewNode>,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
}

impl CompositionPreview {
    pub fn ready(&self) -> bool {
        self.blockers.is_empty()
    }
}

/// Resolve a profile composition without packaging, downloading, installing, or writing state.
pub fn preview_profile_composition(
    application_executable: &Path,
    workspace_root: Option<&Path>,
    selector: &str,
) -> Result<CompositionPreview, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let paths = workspace_root
        .map(|root| EnvironmentPaths::from_installation_and_source_path(installation.clone(), root))
        .transpose()?;
    let plan = content::preview_install(&installation, paths.as_ref(), selector)?;
    let mut blockers = plan.blockers;
    if let Some(root) = plan.nodes.last()
        && root.kind != vapor_core::ContentKind::Packagepack.as_str()
    {
        blockers.push(format!(
            "A launchable composition must be rooted in a Packagepack; '{}' is a {}",
            root.id, root.kind,
        ));
    }
    let source = |source| match source {
        content::InstallPlanSource::Workspace => CompositionPreviewSource::Workspace,
        content::InstallPlanSource::Cache => CompositionPreviewSource::Cache,
        content::InstallPlanSource::Installed => CompositionPreviewSource::Installed,
        content::InstallPlanSource::RootSeed => CompositionPreviewSource::DownloadRequired,
    };
    Ok(CompositionPreview {
        selector: plan.selector,
        nodes: plan
            .nodes
            .into_iter()
            .map(|node| CompositionPreviewNode {
                id: node.id,
                version: node.version,
                kind: node.kind,
                published_file_id: node.published_file_id,
                source: source(node.source),
                already_installed: node.already_installed,
                dependencies: node
                    .dependencies
                    .into_iter()
                    .map(|dependency| CompositionPreviewDependency {
                        id: dependency.id,
                        relationship: dependency.relationship,
                        version: dependency.version,
                        optional: dependency.optional,
                    })
                    .collect(),
            })
            .collect(),
        blockers,
        warnings: plan.warnings,
    })
}

/// Resolve a root-declared launch alias to its Packagepack content ID.
pub fn default_packagepack_selector(
    application_executable: &Path,
    launch: &str,
) -> Result<String, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    content::default_packagepack_id(&installation, launch)
}

/// Download a public Workshop item by PublishedFileId into Vapor's app-owned cache.
pub fn download_workshop_item(
    application_executable: &Path,
    published_file_id: &str,
) -> Result<WorkshopAcquisition, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let report = content::download_many(
        &installation,
        None,
        &[published_file_id.trim().to_owned()],
        None,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "Workshop download returned no artifact".to_owned())?;
    let cached = content::cached_artifact(&installation, report.artifact_id())?;
    Ok(WorkshopAcquisition {
        id: report.artifact_id().to_owned(),
        version: cached.version().map(ToOwned::to_owned),
        kind: cached.kind().to_owned(),
        cache_root: report.cache_root().to_path_buf(),
        fingerprint: format!(
            "{}:{}",
            report.fingerprint().algorithm(),
            report.fingerprint().hash()
        ),
    })
}

pub fn verify_installation(
    application_executable: &Path,
    root_selector: &str,
    selectors: &[String],
) -> Result<InstallationCheck, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let mut ok = !selectors.is_empty();
    let mut present = Vec::new();
    let mut detail = Vec::new();
    for selector in selectors {
        match content::verify(&installation, Some(selector)) {
            Ok(reports) => {
                for report in reports {
                    if !report.ok() {
                        ok = false;
                    }
                    detail.push(format!("{}: {}", report.artifact_id(), report.detail()));
                }
                if content::has_installed_artifact(&installation, selector)? {
                    present.push(selector.clone());
                }
            }
            Err(error) => {
                ok = false;
                let installed = content::has_installed_artifact(&installation, selector)?;
                detail.push(if installed {
                    format!("{selector}: verification failed: {error}")
                } else {
                    format!("{selector}: not installed at the selected version")
                });
            }
        }
    }
    let mut artifacts = Vec::new();
    for artifact in content::installed_artifacts(&installation, &present)? {
        if !artifact.enabled() {
            ok = false;
            detail.push(format!("{}: disabled", artifact.artifact_id()));
        }
        artifacts.push(InstalledArtifact {
            id: artifact.artifact_id().to_owned(),
            version: artifact.version().map(ToOwned::to_owned),
            root: artifact.installed_root().to_path_buf(),
            fingerprint: format!(
                "{}:{}",
                artifact.fingerprint().algorithm(),
                artifact.fingerprint().hash()
            ),
        });
    }
    let graph = content::installed_packagepack_graph(&installation, root_selector)
        .ok()
        .map(|graph| snapshot_installed_graph(&graph));
    Ok(InstallationCheck {
        ok,
        detail,
        artifacts,
        graph,
    })
}

pub fn repair_profile_composition(
    application_executable: &Path,
    workspace_root: Option<&Path>,
    selector: &str,
    graph_selectors: &[String],
) -> Result<InstalledComposition, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let paths = workspace_root
        .map(|root| EnvironmentPaths::from_installation_and_source_path(installation.clone(), root))
        .transpose()?;
    let identities = if graph_selectors.is_empty() {
        vec![selector.to_owned()]
    } else {
        graph_selectors.to_vec()
    };
    for identity in &identities {
        if content::has_installed_artifact(&installation, identity)? {
            content::repair(&installation, paths.as_ref(), Some(identity))?;
        } else {
            content::install(&installation, paths.as_ref(), identity)?;
        }
    }
    installed_composition(&installation, selector)
}

/// Install one profile root and its dependencies through Vapor Shell's
/// app-owned cache, validation, receipts, and installation state.
pub fn install_profile_composition(
    application_executable: &Path,
    workspace_root: Option<&Path>,
    selector: &str,
) -> Result<InstalledComposition, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let paths = workspace_root
        .map(|root| EnvironmentPaths::from_installation_and_source_path(installation.clone(), root))
        .transpose()?;
    content::install(&installation, paths.as_ref(), selector)?;
    installed_composition(&installation, selector)
}

/// Update one profile root from its current workspace source or Vapor cache.
pub fn update_profile_composition(
    application_executable: &Path,
    workspace_root: Option<&Path>,
    selector: &str,
) -> Result<InstalledComposition, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let paths = workspace_root
        .map(|root| EnvironmentPaths::from_installation_and_source_path(installation.clone(), root))
        .transpose()?;
    content::update(&installation, paths.as_ref(), Some(selector))?;
    installed_composition(&installation, selector)
}

/// Launch one installed profile composition without changing Shell's global selection.
pub fn run_installed_profile(
    application_executable: &Path,
    workspace_root: Option<&Path>,
    selector: &str,
    arguments: &[String],
) -> Result<String, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    if !content::has_installed_artifact(&installation, selector)? {
        let paths = workspace_root
            .map(|root| {
                EnvironmentPaths::from_installation_and_source_path(installation.clone(), root)
            })
            .transpose()?;
        content::install(&installation, paths.as_ref(), selector)?;
    }
    let plan = content::prepare_packagepack_launch(&installation, selector)?;
    let summary = format!(
        "Launched {} using {}",
        plan.packagepack_id(),
        plan.engine_id()
    );
    content::run_packagepack_launch(&plan, "studio-profile", arguments)?;
    Ok(summary)
}

/// Install source changes for one profile, then launch that exact installed graph.
pub fn run_source_profile(
    application_executable: &Path,
    workspace_root: &Path,
    selector: &str,
    arguments: &[String],
) -> Result<InstalledComposition, String> {
    let composition =
        install_profile_composition(application_executable, Some(workspace_root), selector)?;
    run_installed_profile(
        application_executable,
        Some(workspace_root),
        selector,
        arguments,
    )?;
    Ok(composition)
}

fn installed_composition(
    installation: &InstallationPaths,
    selector: &str,
) -> Result<InstalledComposition, String> {
    let graph = content::installed_packagepack_graph(installation, selector)?;
    let graph_snapshot = snapshot_installed_graph(&graph);
    let selectors: Vec<_> = graph
        .nodes()
        .iter()
        .map(|artifact| {
            artifact
                .version()
                .map(|version| format!("{}@{version}", artifact.artifact_id()))
                .unwrap_or_else(|| artifact.artifact_id().to_owned())
        })
        .collect();
    let artifacts = installed_snapshot(installation, &selectors)?;
    Ok(InstalledComposition {
        graph: graph_snapshot,
        artifacts,
    })
}

fn snapshot_installed_graph(
    graph: &content::InstalledPackagepackGraph,
) -> crate::ResolvedCompositionGraph {
    let identity = |artifact: &content::InstalledArtifactSnapshot| crate::ContentIdentity {
        id: artifact.artifact_id().to_owned(),
        version: artifact.version().map(ToOwned::to_owned),
    };
    let by_id: std::collections::BTreeMap<_, _> = graph
        .nodes()
        .iter()
        .map(|artifact| (artifact.artifact_id(), artifact))
        .collect();
    let nodes = graph
        .nodes()
        .iter()
        .map(|artifact| {
            let dependencies = artifact
                .dependencies()
                .iter()
                .filter(|dependency| dependency.relationship() != "conflict")
                .filter_map(|dependency| {
                    by_id
                        .get(dependency.id())
                        .map(|target| crate::ResolvedDependencyIdentity {
                            binding: dependency.relationship().to_owned(),
                            content: identity(target),
                        })
                })
                .collect();
            crate::ResolvedNodeIdentity {
                content: identity(artifact),
                kind: artifact.kind().to_owned(),
                fingerprint: Some(format!(
                    "{}:{}",
                    artifact.fingerprint().algorithm(),
                    artifact.fingerprint().hash()
                )),
                source: Some(artifact.source().to_owned()),
                dependencies,
            }
        })
        .collect();
    crate::ResolvedCompositionGraph {
        root: identity(graph.root()),
        effective_engine: identity(graph.engine()),
        effective_game: graph.game().map(identity),
        nodes,
    }
}

fn installed_snapshot(
    installation: &InstallationPaths,
    identities: &[String],
) -> Result<Vec<InstalledArtifact>, String> {
    content::installed_artifacts(installation, identities)?
        .into_iter()
        .map(|artifact| {
            if !artifact.enabled() {
                return Err(format!(
                    "{} is installed but disabled; enable it before using this composition",
                    artifact.artifact_id()
                ));
            }
            Ok(InstalledArtifact {
                id: artifact.artifact_id().to_owned(),
                version: artifact.version().map(ToOwned::to_owned),
                root: artifact.installed_root().to_path_buf(),
                fingerprint: format!(
                    "{}:{}",
                    artifact.fingerprint().algorithm(),
                    artifact.fingerprint().hash()
                ),
            })
        })
        .collect()
}
