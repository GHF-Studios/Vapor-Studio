#[path = "app_shell.rs"]
mod app_shell;
#[path = "authoring_results.rs"]
mod authoring_results;
#[path = "catalog.rs"]
mod catalog;
#[path = "components/mod.rs"]
mod components;
#[path = "composition_lifecycle.rs"]
mod composition_lifecycle;
#[path = "content_creation.rs"]
mod content_creation;
#[path = "directory_picker.rs"]
mod directory_picker;
#[path = "operation_center.rs"]
mod operation_center;
#[path = "operation_protocol.rs"]
mod operation_protocol;
#[path = "operation_results.rs"]
mod operation_results;
#[path = "operations.rs"]
mod operations;
#[path = "pages/mod.rs"]
mod pages;
#[path = "profiles.rs"]
mod profiles;
#[path = "root_publishing.rs"]
mod root_publishing;
#[path = "source_authoring.rs"]
mod source_authoring;
#[path = "workshop.rs"]
mod workshop;
#[path = "workspace_content.rs"]
mod workspace_content;
#[path = "workspace_editor.rs"]
mod workspace_editor;
#[path = "workspace_files.rs"]
mod workspace_files;
#[path = "workspace_ops.rs"]
mod workspace_ops;

use crate::navigation::Section;
use catalog::{
    CatalogFilter, CatalogState, catalog_content_selector, catalog_content_selector_matches,
    content_selector, source_content_kind, source_content_selector,
    source_content_selector_matches,
};
use composition_lifecycle::ProfileCompositionState;
use directory_picker::DirectoryPicker;
use eframe::egui;
use operations::OperationState;
use profiles::ProfileState;
use root_publishing::RootPublishingState;
use source_authoring::AuthoringState;
use std::path::PathBuf;
use vapor_studio_core::decode_profiles;
use workshop::WorkshopState;
use workspace_ops::WorkspaceState;

pub struct StudioApp {
    section: Section,
    workspace: WorkspaceState,
    profile: ProfileState,
    catalog: CatalogState,
    authoring: AuthoringState,
    profile_composition: ProfileCompositionState,
    workshop: WorkshopState,
    root_publishing: RootPublishingState,
    status: String,
    operation: OperationState,
    diagnostics: vapor_devtools::app::DevtoolsApp,
    directory_picker: Option<DirectoryPicker>,
    command_palette_open: bool,
    command_palette_search: String,
    command_palette_focus_search: bool,
}

impl StudioApp {
    pub fn new(
        creation: &eframe::CreationContext<'_>,
        workspace: Option<PathBuf>,
        play_mode: bool,
    ) -> Self {
        configure_studio_visuals(&creation.egui_ctx);
        let saved_workspace = creation
            .storage
            .and_then(|storage| storage.get_string("workspace_root"));
        let saved_editor_drafts = creation
            .storage
            .and_then(|storage| storage.get_string("workspace_editor_drafts"));
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
        let saved_operation_history = creation
            .storage
            .and_then(|storage| storage.get_string("operation_history"));
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
        let mut workspace_state = WorkspaceState::new(workspace_root, workspace_root_draft, root);
        workspace_state.editor.draft_recovery = saved_editor_drafts;
        let mut operation = OperationState::default();
        operation.restore_history(saved_operation_history.as_deref());
        let mut app = Self {
            section: if play_mode {
                Section::Library
            } else {
                Section::Home
            },
            workspace: workspace_state,
            profile: ProfileState::new(
                profiles,
                active_profile,
                profile_name_draft,
                profile_recovery
                    .as_ref()
                    .map(|recovery| recovery.message.clone()),
                if profile_recovery.is_some() {
                    saved_profiles
                } else {
                    None
                },
            ),
            catalog: CatalogState::default(),
            workshop: WorkshopState::default(),
            root_publishing: RootPublishingState::default(),
            authoring: AuthoringState::default(),
            profile_composition: ProfileCompositionState::default(),
            status: "Choose a workspace or discover content in this folder.".to_owned(),
            operation,
            diagnostics: vapor_devtools::connect(None),
            directory_picker: None,
            command_palette_open: false,
            command_palette_search: String::new(),
            command_palette_focus_search: false,
        };
        app.refresh_registered_sources();
        if play_mode {
            app.launch_default_profile();
        } else if let Some(workspace) = requested_workspace {
            app.open_workspace_path_to(workspace, Section::Home);
        } else {
            app.discover();
        }
        app
    }
}

fn configure_studio_visuals(context: &egui::Context) {
    use components::design::{ACCENT, ACCENT_FILL};

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(15, 20, 28);
    visuals.window_fill = egui::Color32::from_rgb(20, 27, 36);
    visuals.extreme_bg_color = egui::Color32::from_rgb(9, 13, 19);
    visuals.faint_bg_color = egui::Color32::from_rgb(23, 31, 41);
    visuals.selection.bg_fill = ACCENT_FILL;
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(18, 25, 34);
    visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(194, 206, 218);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(29, 39, 51);
    visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(216, 225, 234);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(39, 59, 75);
    visuals.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;
    visuals.widgets.active.bg_fill = ACCENT_FILL;
    visuals.widgets.active.fg_stroke.color = egui::Color32::WHITE;
    visuals.widgets.open.bg_fill = egui::Color32::from_rgb(31, 51, 65);
    visuals.widgets.open.fg_stroke.color = egui::Color32::from_rgb(224, 235, 243);
    visuals.widgets.noninteractive.bg_stroke.color = egui::Color32::from_rgb(42, 54, 67);
    visuals.widgets.inactive.bg_stroke.color = egui::Color32::from_rgb(53, 69, 85);
    visuals.widgets.hovered.bg_stroke.color = egui::Color32::from_rgb(79, 137, 159);
    visuals.widgets.active.bg_stroke.color = ACCENT;
    let mut style = (*context.style_of(egui::Theme::Dark)).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.indent = 18.0;
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(24.0));
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    context.set_style_of(egui::Theme::Dark, style);
    context.set_visuals(visuals);
}
