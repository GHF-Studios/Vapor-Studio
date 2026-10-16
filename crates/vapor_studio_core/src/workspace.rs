use std::path::{Path, PathBuf};
use vapor_shell::{
    content,
    discovery::{EnvironmentPaths, InstallationPaths},
    manifest::ContentKind as ShellContentKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceContent {
    pub id: String,
    pub version: Option<String>,
    pub kind: String,
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub dependencies: Vec<String>,
}

pub use vapor_shell::content::{CompositionLink as SourceCompositionLink, SourceComposition};

pub fn load_source_composition(
    application_executable: &Path,
    workspace_root: &Path,
    selector: &str,
) -> Result<SourceComposition, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let paths = EnvironmentPaths::from_installation_and_source_path(installation, workspace_root)?;
    content::load_source_composition(&paths, selector)
}

pub fn save_source_composition(
    application_executable: &Path,
    workspace_root: &Path,
    selector: &str,
    expected_source: &str,
    links: &[SourceCompositionLink],
) -> Result<SourceComposition, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let paths = EnvironmentPaths::from_installation_and_source_path(installation, workspace_root)?;
    content::save_source_composition(&paths, selector, expected_source, links)
}

/// Ask Vapor Shell whether one source role can directly depend on another.
pub fn source_dependency_allowed(
    parent: vapor_core::ContentKind,
    child: vapor_core::ContentKind,
) -> bool {
    let Some(parent) = shell_content_kind(parent) else {
        return false;
    };
    let Some(child) = shell_content_kind(child) else {
        return false;
    };
    content::source_dependency_allowed(parent, child)
}

pub fn discover_source_content(
    application_executable: &Path,
    workspace_root: &Path,
) -> Result<Vec<SourceContent>, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let paths = EnvironmentPaths::from_installation_and_source_path(installation, workspace_root)?;
    Ok(content::discover(&paths)?
        .artifacts()
        .iter()
        .map(|artifact| SourceContent {
            id: artifact.id().to_owned(),
            version: artifact.version().map(ToOwned::to_owned),
            kind: artifact.kind().to_string(),
            root: artifact.root().to_path_buf(),
            manifest: artifact.manifest().to_path_buf(),
            dependencies: artifact
                .dependencies()
                .iter()
                .map(|dependency| format!("{} · {}", dependency.relationship(), dependency.id()))
                .collect(),
        })
        .collect())
}

pub fn validate_profile_source(
    application_executable: &Path,
    workspace_root: &Path,
    selector: &str,
) -> Result<String, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let paths = EnvironmentPaths::from_installation_and_source_path(installation, workspace_root)?;
    let report = content::validate(&paths, Some(selector))?;
    Ok(format!(
        "Validated {} source artifact{}; {} diagnostic{}.",
        report.checked().len(),
        if report.checked().len() == 1 { "" } else { "s" },
        report.diagnostics().len(),
        if report.diagnostics().len() == 1 {
            ""
        } else {
            "s"
        },
    ))
}

/// Validate every authored content project in a workspace without building it.
pub fn validate_workspace_source(
    application_executable: &Path,
    workspace_root: &Path,
) -> Result<String, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let paths = EnvironmentPaths::from_installation_and_source_path(installation, workspace_root)?;
    let report = content::validate(&paths, None)?;
    let mut summary = format!(
        "Validated {} authored content project{}.",
        report.checked().len(),
        if report.checked().len() == 1 { "" } else { "s" },
    );
    for diagnostic in report.diagnostics() {
        summary.push_str("\n• ");
        summary.push_str(diagnostic);
    }
    Ok(summary)
}

pub fn create_content_workspace(
    application_executable: &Path,
    workspace_root: &Path,
    id: &str,
    version: &str,
    kind: vapor_core::ContentKind,
    dependency_selectors: &[String],
) -> Result<PathBuf, String> {
    let kind = match shell_content_kind(kind) {
        Some(kind) => kind,
        None => {
            return Err("Library content does not yet have a Vapor Shell manifest role".to_owned());
        }
    };
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let paths = EnvironmentPaths::from_installation_and_source_path(installation, workspace_root)?;
    content::create_source_project(&paths, id, version, kind, dependency_selectors)
}

/// Scaffold the maintained Engine/Game/Packagepack starter workspace.
pub fn create_basic_content_workspace(
    application_executable: &Path,
    destination: &Path,
    organization: &str,
    name: &str,
) -> Result<PathBuf, String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    content::create_basic_content_workspace(
        &installation,
        destination.to_path_buf(),
        organization,
        name,
    )
}

fn shell_content_kind(kind: vapor_core::ContentKind) -> Option<ShellContentKind> {
    Some(match kind {
        vapor_core::ContentKind::Engine => ShellContentKind::Engine,
        vapor_core::ContentKind::Game => ShellContentKind::Game,
        vapor_core::ContentKind::Packagepack => ShellContentKind::Packagepack,
        vapor_core::ContentKind::Enginepack => ShellContentKind::Enginepack,
        vapor_core::ContentKind::Gamepack => ShellContentKind::Gamepack,
        vapor_core::ContentKind::Modpack => ShellContentKind::Modpack,
        vapor_core::ContentKind::EngineMod => ShellContentKind::EngineMod,
        vapor_core::ContentKind::GameMod => ShellContentKind::GameMod,
        vapor_core::ContentKind::ExtensionMod => ShellContentKind::ExtensionMod,
        vapor_core::ContentKind::Library => return None,
    })
}

/// Add a validated authored Workspace to this Vapor Installation's known sources.
pub fn remember_workspace(workspace_root: &Path) -> Result<Vec<PathBuf>, String> {
    let installation = vapor_core::VaporInstallation::discover()
        .map_err(|error| error.to_string())?;
    let state = vapor_core::remember_source(&installation, workspace_root)
        .map_err(|error| error.to_string())?;
    Ok(state.known)
}

/// Register an existing content project through Vapor Shell without moving it.
pub fn adopt_workspace_content_project(
    application_executable: &Path,
    workspace_root: &Path,
    project_path: &Path,
) -> Result<(String, Vec<SourceContent>), String> {
    let installation = InstallationPaths::from_application_executable(application_executable)?;
    let paths = EnvironmentPaths::from_installation_and_source_path(installation, workspace_root)?;
    let summary = content::adopt_source_project(&paths, project_path)?;
    let contents = discover_source_content(application_executable, workspace_root)?;
    Ok((summary, contents))
}
