#[path = "app_shell.rs"]
mod app_shell;
#[path = "authoring_results.rs"]
mod authoring_results;
#[path = "catalog.rs"]
mod catalog;
#[path = "command_palette.rs"]
mod command_palette;
#[path = "components/mod.rs"]
mod components;
#[path = "composition_lifecycle.rs"]
mod composition_lifecycle;
#[path = "compositions.rs"]
mod compositions;
#[path = "content_creation.rs"]
mod content_creation;
#[path = "directory_picker.rs"]
mod directory_picker;
#[path = "operation_center.rs"]
mod operation_center;
#[path = "operation_protocol.rs"]
mod operation_protocol;
#[path = "operations.rs"]
mod operations;
#[path = "pages/mod.rs"]
mod pages;
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
#[path = "workspace_ops.rs"]
mod workspace_ops;

use crate::navigation::Section;
use catalog::{
    CatalogFilter, CatalogState, catalog_content_selector, catalog_content_selector_matches,
    content_kind_name, content_selector, display_catalog_identity, display_catalog_name,
    display_content_kind, source_content_kind, source_content_selector,
    source_content_selector_matches,
};
use composition_lifecycle::CompositionPreviewState;
use compositions::CompositionState;
use directory_picker::DirectoryPicker;
use eframe::egui;
use operations::OperationState;
use root_publishing::RootPublishingState;
use source_authoring::AuthoringState;
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};
use vapor_client::{Client, ClientHostConfig, service_sibling_path};
use vapor_client_application::decode_compositions;
use vapor_client_protocol::{ClientAction, LegacyStorageSeed};
use workshop::WorkshopState;
use workspace_ops::WorkspaceState;

pub struct StudioApp {
    section: Section,
    workspace: WorkspaceState,
    composition: CompositionState,
    catalog: CatalogState,
    authoring: AuthoringState,
    composition_preview: CompositionPreviewState,
    workshop: WorkshopState,
    root_publishing: RootPublishingState,
    status: String,
    operation: OperationState,
    service: Option<Client>,
    service_error: Option<String>,
    service_connection_config: Option<(PathBuf, ClientHostConfig)>,
    service_connection_attempt: Option<Receiver<Result<Client, String>>>,
    diagnostics: vapor_devtools::app::DevtoolsApp,
    directory_picker: Option<DirectoryPicker>,
    command_palette_open: bool,
    command_palette_search: String,
    command_palette_focus_search: bool,
    command_palette_selected_index: usize,
    console_open: bool,
    console_problems_active: bool,
}

impl StudioApp {
    pub(super) fn service_action_enabled(&self, action_id: &str) -> Option<bool> {
        self.service
            .as_ref()?
            .latest_snapshot()
            .available_actions
            .iter()
            .find(|action| action.id == action_id)
            .map(|action| action.enabled || action.queueable)
    }

    pub(super) fn service_action_reason(&self, action_id: &str) -> Option<&str> {
        self.service
            .as_ref()?
            .latest_snapshot()
            .available_actions
            .iter()
            .find(|action| action.id == action_id)?
            .reason
            .as_deref()
    }

    pub fn new(creation: &eframe::CreationContext<'_>, workspace: Option<PathBuf>) -> Self {
        configure_studio_visuals(&creation.egui_ctx);
        let saved_workspace = creation
            .storage
            .and_then(|storage| storage.get_string("workspace_root"));
        let saved_editor_drafts = creation
            .storage
            .and_then(|storage| storage.get_string("workspace_editor_drafts"));
        let explicit_workspace = workspace.clone();
        let requested_workspace = workspace
            .or_else(|| saved_workspace.clone().map(PathBuf::from))
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .as_ref()
                    .filter(|root| root.join("Workspace.vapor.toml").is_file())
                    .cloned()
            });
        let fallback_root = home_directory().join("Vapor");
        let saved_compositions = creation.storage.and_then(|storage| {
            storage
                .get_string("profiles")
                .or_else(|| storage.get_string("compositions"))
        });
        let (composition_document, composition_recovery) =
            decode_compositions(saved_compositions.as_deref());
        let compositions = composition_document.compositions;
        let saved_composition_id = creation.storage.and_then(|storage| {
            storage
                .get_string("active_profile_id")
                .or_else(|| storage.get_string("active_composition_id"))
        });
        let saved_operation_history = creation
            .storage
            .and_then(|storage| storage.get_string("operation_history"));
        let legacy_storage = LegacyStorageSeed {
            profiles: saved_compositions.clone(),
            active_profile_id: saved_composition_id.clone(),
            active_profile_index: creation
                .storage
                .and_then(|storage| storage.get_string("active_profile")),
            workspace_root: saved_workspace.clone(),
            editor_drafts: saved_editor_drafts.clone(),
            operation_history: saved_operation_history.clone(),
            recovery_backup: creation.storage.and_then(|storage| {
                storage
                    .get_string("profiles.recovery_backup")
                    .or_else(|| storage.get_string("compositions.recovery_backup"))
            }),
        };
        let active_composition = saved_composition_id
            .as_deref()
            .and_then(|id| {
                compositions
                    .iter()
                    .position(|composition| composition.id.as_str() == id)
            })
            .or_else(|| {
                creation
                    .storage
                    .and_then(|storage| {
                        storage
                            .get_string("active_profile")
                            .or_else(|| storage.get_string("active_composition"))
                    })
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|index| *index < compositions.len())
            })
            .unwrap_or(0);
        let workspace_root = requested_workspace
            .as_ref()
            .map(|workspace| workspace.display().to_string())
            .unwrap_or_default();
        let workspace_root_draft = workspace_root.clone();
        let composition_name_draft = compositions[active_composition].name.clone();
        let mut workspace_state =
            WorkspaceState::new(workspace_root, workspace_root_draft, fallback_root);
        workspace_state.editor.draft_recovery = saved_editor_drafts;
        let mut operation = OperationState::default();
        operation.restore_history(saved_operation_history.as_deref());
        let mut app = Self {
            section: Section::Home,
            workspace: workspace_state,
            composition: CompositionState::new(
                compositions,
                active_composition,
                composition_name_draft,
                composition_recovery
                    .as_ref()
                    .map(|recovery| recovery.message.clone()),
                if composition_recovery.is_some() {
                    saved_compositions
                } else {
                    None
                },
            ),
            catalog: CatalogState::default(),
            workshop: WorkshopState::default(),
            root_publishing: RootPublishingState::default(),
            authoring: AuthoringState::default(),
            composition_preview: CompositionPreviewState::default(),
            status: "Choose a workspace to manage authored content. Use Vapor Launcher to play compositions.".to_owned(),
            operation,
            service: None,
            service_error: None,
            service_connection_config: None,
            service_connection_attempt: None,
            diagnostics: vapor_devtools::app::DevtoolsApp::new(),
            directory_picker: None,
            command_palette_open: false,
            command_palette_search: String::new(),
            command_palette_focus_search: false,
            command_palette_selected_index: 0,
            console_open: false,
            console_problems_active: false,
        };
        app.start_service(
            explicit_workspace,
            Some(
                app.composition.items[app.composition.active_index]
                    .id
                    .clone(),
            ),
            legacy_storage,
        );
        app.refresh_registered_sources();
        if app.composition.items[app.composition.active_index].id == "default"
            && app.composition.items[app.composition.active_index]
                .root_package
                .is_none()
            && app.composition.items[app.composition.active_index]
                .workspace
                .is_none()
        {
            app.ensure_default_composition_root();
        }
        if let Some(workspace) = requested_workspace {
            app.open_workspace_path_to(workspace, Section::Home);
        } else {
            app.discover();
        }
        app
    }

    fn start_service(
        &mut self,
        workspace_root: Option<PathBuf>,
        active_composition_id: Option<String>,
        legacy_storage: LegacyStorageSeed,
    ) {
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                self.service_error =
                    Some(format!("Could not locate the Studio executable: {error}"));
                self.catalog.catalog_discovery_error = self.service_error.clone();
                return;
            }
        };
        let service_path = service_sibling_path(&executable);
        let bootstrap = ClientHostConfig {
            application_executable: executable,
            workshop_app_id: None,
            workspace_root,
            active_composition_id,
            fixture_mode: false,
            legacy_storage,
            state_directory: None,
        };
        self.service_connection_config = Some((service_path.clone(), bootstrap.clone()));
        match Client::connect_or_start(&service_path, bootstrap) {
            Ok(mut client) => {
                if let Err(error) = client.send(
                    ClientAction::Developer(
                        vapor_client_protocol::DeveloperAction::SubscribeTelemetry {
                            enabled: true,
                        },
                    ),
                    None,
                ) {
                    self.status = format!("Live diagnostics are unavailable: {error}");
                }
                let initial_snapshot = client.latest_snapshot().clone();
                self.service = Some(client);
                self.service_error = None;
                self.apply_service_catalog_snapshot(&initial_snapshot);
                self.workspace.editor.draft_recovery =
                    initial_snapshot.durable_state.editor_drafts.clone();
                self.operation
                    .restore_history(initial_snapshot.durable_state.operation_history.as_deref());
                self.submit_service_action(ClientAction::Developer(
                    vapor_client_protocol::DeveloperAction::RefreshProfileTraces,
                ));
                self.discover_superworkspace_projects_if_ready();
            }
            Err(error) => {
                let message = match &error {
                    vapor_client::ClientError::Spawn(detail)
                        if detail.contains("os error 2") || detail.contains("No such file") =>
                    {
                        eprintln!("[vapor-studio] background service failed to start: {detail}");
                        "Studio could not start its background component. Repair or update Vapor Studio, then try again.".to_owned()
                    }
                    _ => format!("Studio could not start a required background component: {error}"),
                };
                self.catalog.catalog_discovery_loaded = false;
                self.catalog.catalog_discovery_error = Some(message.clone());
                self.service_error = Some(message);
            }
        }
    }

    pub(super) fn reconnect_service_in_background(&mut self) {
        if self.service.is_some() || self.service_connection_attempt.is_some() {
            return;
        }
        let Some((service_path, bootstrap)) = self.service_connection_config.clone() else {
            self.service_error = Some("Vapor cannot reconnect from this launch.".to_owned());
            return;
        };
        let (sender, receiver) = mpsc::channel();
        let attempt = thread::Builder::new()
            .name("vapor-studio-service-reconnect".to_owned())
            .spawn(move || {
                let result = Client::connect_or_start(&service_path, bootstrap)
                    .map_err(|error| error.to_string());
                let _ = sender.send(result);
            });
        match attempt {
            Ok(_) => {
                self.service_connection_attempt = Some(receiver);
                self.service_error = None;
                self.status = "Reconnecting to Vapor…".to_owned();
            }
            Err(error) => {
                eprintln!("[vapor-studio] could not start reconnect attempt: {error}");
                let message = "Vapor is unavailable. Try reconnecting.".to_owned();
                self.service_error = Some(message.clone());
                self.catalog.catalog_discovery_error = Some(message);
            }
        }
    }

    pub(super) fn poll_service_connection(&mut self) {
        let result = self
            .service_connection_attempt
            .as_ref()
            .map(Receiver::try_recv);
        match result {
            Some(Ok(Ok(client))) => {
                self.service_connection_attempt = None;
                let snapshot = client.latest_snapshot().clone();
                self.service = Some(client);
                self.apply_service_catalog_snapshot(&snapshot);
                self.service_error = None;
                self.status =
                    "Vapor is connected again. Check the last action before trying it again."
                        .to_owned();
                self.submit_service_action(ClientAction::Catalog(
                    vapor_client_protocol::CatalogAction::Refresh,
                ));
                self.submit_service_action(ClientAction::Developer(
                    vapor_client_protocol::DeveloperAction::SubscribeTelemetry { enabled: true },
                ));
                self.submit_service_action(ClientAction::Developer(
                    vapor_client_protocol::DeveloperAction::RefreshProfileTraces,
                ));
                self.discover_superworkspace_projects_if_ready();
            }
            Some(Ok(Err(error))) => {
                self.service_connection_attempt = None;
                eprintln!("[vapor-studio] service reconnect failed: {error}");
                let message = "Vapor is unavailable. Try reconnecting.".to_owned();
                self.service_error = Some(message.clone());
                self.catalog.catalog_discovery_error = Some(message);
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.service_connection_attempt = None;
                let message = "Vapor could not finish reconnecting. Try again.".to_owned();
                self.service_error = Some(message.clone());
                self.catalog.catalog_discovery_error = Some(message);
            }
            Some(Err(TryRecvError::Empty)) | None => {}
        }
    }

    pub(super) fn discover_superworkspace_projects_if_ready(&mut self) {
        let action = ClientAction::Workspace(
            vapor_client_protocol::WorkspaceAction::DiscoverSuperworkspaceRepositories,
        );
        let is_ready = self.service.as_ref().is_some_and(|service| {
            let snapshot = service.latest_snapshot();
            snapshot.superworkspace.status == "ready"
                && !snapshot.superworkspace.available_repositories_loaded
                && snapshot
                    .superworkspace
                    .available_repositories_error
                    .is_none()
        });
        if is_ready && !self.operation.service_action_is_pending_or_active(&action) {
            self.start_service_operation(
                action,
                None,
                vapor_client_application::OperationKind::Discover,
                vapor_client_application::OperationPhase::Resolving,
                "Finding available Vapor source projects…",
            );
        }
    }
}

fn home_directory() -> PathBuf {
    #[cfg(windows)]
    let variable = "USERPROFILE";
    #[cfg(not(windows))]
    let variable = "HOME";

    std::env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

fn configure_studio_visuals(context: &egui::Context) {
    use components::design::{ACCENT, ACCENT_FILL};

    // Keep Studio's authored palette consistent across platforms. The system
    // theme is often light on Windows, which otherwise selects egui's default
    // light visuals even though the Studio palette is designed for dark mode.
    context.set_theme(egui::ThemePreference::Dark);
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
    context.set_visuals_of(egui::Theme::Dark, visuals);
}
