#[path = "components/mod.rs"]
mod components;
#[path = "app_shell.rs"]
mod app_shell;
#[path = "catalog.rs"]
mod catalog;
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
#[path = "workspace_editor.rs"]
mod workspace_editor;
#[path = "workshop.rs"]
mod workshop;

use crate::navigation::Section;
use std::path::PathBuf;
use operations::OperationState;
use profiles::ProfileState;
use source_authoring::AuthoringState;
use composition_lifecycle::ProfileCompositionState;
use workshop::WorkshopState;
use workspace_ops::WorkspaceState;
use vapor_studio_core::decode_profiles;
use catalog::{
    CatalogState, catalog_content_selector, catalog_matches_filters, content_selector,
    source_content_kind, source_content_selector,
};

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
