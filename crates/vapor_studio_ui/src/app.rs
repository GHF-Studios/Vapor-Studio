#[path = "components/mod.rs"]
mod components;
#[path = "app_shell.rs"]
mod app_shell;
#[path = "composition_lifecycle.rs"]
mod composition_lifecycle;
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
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc::Receiver};
use operations::OperationResult;
use workshop::WorkshopState;
use workspace_ops::WorkspaceState;
use vapor_core::{ContentKind, OperationEvent as CoreOperationEvent};
use vapor_studio_core::{
    CompositionProfile, ContentIdentity, OperationEvent, OperationId, OperationKind,
    decode_profiles,
};

pub struct StudioApp {
    section: Section,
    workspace: WorkspaceState,
    profiles: Vec<CompositionProfile>,
    profile_recovery: Option<String>,
    profile_recovery_backup: Option<String>,
    active_profile: usize,
    profile_name_draft: String,
    confirm_profile_removal: bool,
    new_profile_name: String,
    search_query: String,
    draft_content_id: String,
    draft_content_version: String,
    draft_content_kind: ContentKind,
    draft_dependencies: Vec<String>,
    draft_workshop_app_id: String,
    draft_workshop_published_file_id: String,
    draft_workshop_visibility: String,
    draft_workshop_title: String,
    draft_workshop_description: String,
    draft_workshop_tags: String,
    draft_workshop_change_note: String,
    composition_edit_selector: String,
    composition_edit: Option<vapor_studio_core::SourceComposition>,
    composition_original_links: Option<Vec<vapor_studio_core::SourceCompositionLink>>,
    composition_link_candidate: String,
    workshop_metadata_edit: Option<vapor_studio_core::SourceWorkshopMetadata>,
    manifest_metadata_dirty: bool,
    runtime_outputs_edit: Option<vapor_studio_core::SourceRuntimeOutputs>,
    runtime_binaries_text: String,
    runtime_libraries_text: String,
    workshop_metadata_app_id: String,
    workshop_metadata_published_file_id: String,
    workshop_metadata_visibility: String,
    workshop_metadata_title: String,
    workshop_metadata_description: String,
    workshop_metadata_tags: String,
    workshop_metadata_change_note: String,
    repreview_after_catalog_refresh: bool,
    contents: Vec<vapor_studio_core::SourceContent>,
    catalog: Vec<vapor_studio_core::CatalogContent>,
    catalog_source_filter: CatalogSourceFilter,
    catalog_kind_filter: Option<String>,
    selected_catalog_content: Option<String>,
    workshop: WorkshopState,
    composition_preview: Option<vapor_studio_core::CompositionPreview>,
    status: String,
    running: bool,
    receiver: Option<Receiver<(OperationId, OperationResult)>>,
    latest_operation_event: Arc<Mutex<Option<CoreOperationEvent>>>,
    active_operation: Option<OperationEvent>,
    operation_history: VecDeque<OperationEvent>,
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
            profiles,
            profile_recovery: profile_recovery
                .as_ref()
                .map(|recovery| recovery.message.clone()),
            profile_recovery_backup: if profile_recovery.is_some() {
                saved_profiles
            } else {
                None
            },
            active_profile,
            profile_name_draft,
            confirm_profile_removal: false,
            new_profile_name: String::new(),
            search_query: String::new(),
            contents: Vec::new(),
            catalog: Vec::new(),
            catalog_source_filter: CatalogSourceFilter::All,
            catalog_kind_filter: None,
            selected_catalog_content: None,
            workshop: WorkshopState::default(),
            composition_preview: None,
            composition_edit_selector: String::new(),
            composition_edit: None,
            composition_original_links: None,
            composition_link_candidate: String::new(),
            workshop_metadata_edit: None,
            manifest_metadata_dirty: false,
            runtime_outputs_edit: None,
            runtime_binaries_text: String::new(),
            runtime_libraries_text: String::new(),
            workshop_metadata_app_id: String::new(),
            workshop_metadata_published_file_id: String::new(),
            workshop_metadata_visibility: String::new(),
            workshop_metadata_title: String::new(),
            workshop_metadata_description: String::new(),
            workshop_metadata_tags: String::new(),
            workshop_metadata_change_note: String::new(),
            repreview_after_catalog_refresh: false,
            draft_content_id: String::new(),
            draft_content_version: "0.1.0".to_owned(),
            draft_content_kind: ContentKind::Packagepack,
            draft_dependencies: Vec::new(),
            draft_workshop_app_id: String::new(),
            draft_workshop_published_file_id: String::new(),
            draft_workshop_visibility: String::new(),
            draft_workshop_title: String::new(),
            draft_workshop_description: String::new(),
            draft_workshop_tags: String::new(),
            draft_workshop_change_note: String::new(),
            status: "Choose a workspace or discover content in this folder.".to_owned(),
            running: false,
            receiver: None,
            latest_operation_event: Arc::new(Mutex::new(None)),
            active_operation: None,
            operation_history: VecDeque::new(),
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
