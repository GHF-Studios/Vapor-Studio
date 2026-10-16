#[path = "components/mod.rs"]
mod components;
#[path = "app_shell.rs"]
mod app_shell;
#[path = "composition_lifecycle.rs"]
mod composition_lifecycle;
#[path = "content_creation.rs"]
mod content_creation;
#[path = "operation_center.rs"]
mod operation_center;
#[path = "operation_results.rs"]
mod operation_results;
#[path = "operations.rs"]
mod operations;
#[path = "profiles.rs"]
mod profiles;
#[path = "source_authoring.rs"]
mod source_authoring;
#[path = "pages/mod.rs"]
mod pages;
#[path = "workspace_files.rs"]
mod workspace_files;
#[path = "workspace_ops.rs"]
mod workspace_ops;
#[path = "workshop.rs"]
mod workshop;

use crate::navigation::{CatalogSourceFilter, Section};
use std::path::PathBuf;
use operations::OperationState;
use profiles::ProfileState;
use source_authoring::AuthoringState;
use composition_lifecycle::ProfileCompositionState;
use workshop::WorkshopState;
use workspace_ops::WorkspaceState;
use vapor_core::ContentKind;
use vapor_studio_core::{
    ContentIdentity, decode_profiles,
};

struct CatalogState {
    search_query: String,
    sources: Vec<vapor_studio_core::SourceContent>,
    items: Vec<vapor_studio_core::CatalogContent>,
    source_filter: CatalogSourceFilter,
    kind_filter: Option<String>,
    selected: Option<String>,
}

impl Default for CatalogState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            sources: Vec::new(),
            items: Vec::new(),
            source_filter: CatalogSourceFilter::All,
            kind_filter: None,
            selected: None,
        }
    }
}

impl CatalogState {
    fn clear_workspace_content(&mut self) {
        self.sources.clear();
        self.items.clear();
        self.selected = None;
    }
}

pub struct StudioApp {
    section: Section,
    workspace: WorkspaceState,
    profile: ProfileState,
    catalog: CatalogState,
    authoring: AuthoringState,
    profile_composition: ProfileCompositionState,
    workshop: WorkshopState,
    status: String,
    operation: OperationState,
    diagnostics: vapor_devtools::app::DevtoolsApp,
}

impl StudioApp {
    pub fn new(
        creation: &eframe::CreationContext<'_>,
        workspace: Option<PathBuf>,
        play_mode: bool,
    ) -> Self {
        let saved_workspace = creation
            .storage
            .and_then(|storage| storage.get_string("workspace_root"));
        let current_directory = std::env::current_dir().ok();
        let requested_workspace = workspace
            .or_else(|| saved_workspace.map(PathBuf::from))
            .or_else(|| {
                current_directory
                    .as_ref()
                    .filter(|root| root.join("Workspace.vapor.toml").is_file())
                    .cloned()
            });
        let root = requested_workspace
            .clone()
            .or(current_directory)
            .unwrap_or_default();
        let saved_profiles = creation
            .storage
            .and_then(|storage| storage.get_string("profiles"));
        let (profile_document, profile_recovery) = decode_profiles(saved_profiles.as_deref());
        let profiles = profile_document.profiles;
        let saved_profile_id = creation
            .storage
            .and_then(|storage| storage.get_string("active_profile_id"));
        let active_profile = saved_profile_id
            .as_deref()
            .and_then(|id| {
                profiles
                    .iter()
                    .position(|profile| profile.id.as_str() == id)
            })
            .or_else(|| {
                creation
                    .storage
                    .and_then(|storage| storage.get_string("active_profile"))
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|index| *index < profiles.len())
            })
            .unwrap_or(0);
        let workspace_root = profiles[active_profile]
            .workspace
            .as_ref()
            .map(|workspace| workspace.root.display().to_string())
            .unwrap_or_default();
        let workspace_root_draft = if workspace_root.is_empty() {
            root.display().to_string()
        } else {
            workspace_root.clone()
        };
        let profile_name_draft = profiles[active_profile].name.clone();
        let mut app = Self {
            section: if play_mode {
                Section::Library
            } else {
                Section::Home
            },
            workspace: WorkspaceState::new(workspace_root, workspace_root_draft, root),
            profile: ProfileState::new(
                profiles,
                active_profile,
                profile_name_draft,
                profile_recovery
                    .as_ref()
                    .map(|recovery| recovery.message.clone()),
                if profile_recovery.is_some() { saved_profiles } else { None },
            ),
            catalog: CatalogState::default(),
            workshop: WorkshopState::default(),
            authoring: AuthoringState::default(),
            profile_composition: ProfileCompositionState::default(),
            status: "Choose a workspace or discover content in this folder.".to_owned(),
            operation: OperationState::default(),
            diagnostics: vapor_devtools::connect(None),
        };
        app.refresh_registered_sources();
        if play_mode {
            app.launch_default_profile();
        } else if let Some(workspace) = requested_workspace {
            app.open_workspace_path(workspace);
        } else {
            app.discover();
        }
        app
    }

}

fn content_selector(identity: &ContentIdentity) -> String {
    identity
        .version
        .as_deref()
        .map(|version| format!("{}@{}", identity.id, version))
        .unwrap_or_else(|| identity.id.clone())
}

pub(crate) fn source_content_selector(content: &vapor_studio_core::SourceContent) -> String {
    content
        .version
        .as_deref()
        .map(|version| format!("{}@{version}", content.id))
        .unwrap_or_else(|| content.id.clone())
}

pub(crate) fn catalog_content_selector(content: &vapor_studio_core::CatalogContent) -> String {
    content_selector(&content.identity())
}

pub(crate) fn source_content_kind(kind: &str) -> Option<ContentKind> {
    Some(match kind {
        "packagepack" => ContentKind::Packagepack,
        "enginepack" => ContentKind::Enginepack,
        "gamepack" => ContentKind::Gamepack,
        "modpack" => ContentKind::Modpack,
        "engine" => ContentKind::Engine,
        "game" => ContentKind::Game,
        "engine-mod" => ContentKind::EngineMod,
        "game-mod" => ContentKind::GameMod,
        "extension-mod" => ContentKind::ExtensionMod,
        _ => return None,
    })
}

pub(super) fn catalog_matches_filters(
    content: &vapor_studio_core::CatalogContent,
    source_filter: CatalogSourceFilter,
    kind_filter: Option<&str>,
    query: &str,
) -> bool {
    let source_matches = match source_filter {
        CatalogSourceFilter::All => true,
        CatalogSourceFilter::Workspace => content
            .locations
            .iter()
            .any(|location| location.kind == vapor_studio_core::CatalogLocationKind::Workspace),
        CatalogSourceFilter::Cache => content
            .locations
            .iter()
            .any(|location| location.kind == vapor_studio_core::CatalogLocationKind::Cache),
        CatalogSourceFilter::Installed => content
            .locations
            .iter()
            .any(|location| location.kind == vapor_studio_core::CatalogLocationKind::Installed),
        CatalogSourceFilter::Workshop => content.published_file_id.is_some(),
    };
    if !source_matches || kind_filter.is_some_and(|kind| content.kind != kind) {
        return false;
    }
    let query = query.trim().to_lowercase();
    query.is_empty()
        || content.id.to_lowercase().contains(&query)
        || content.kind.to_lowercase().contains(&query)
        || content
            .version
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .contains(&query)
        || content
            .published_file_id
            .as_deref()
            .unwrap_or_default()
            .contains(&query)
}
