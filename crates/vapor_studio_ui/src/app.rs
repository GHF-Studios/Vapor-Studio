#[path = "components/mod.rs"]
mod components;
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
use eframe::egui;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc::Receiver};
use operations::OperationResult;
use vapor_core::{ContentKind, OperationEvent as CoreOperationEvent};
use vapor_studio_core::{
    CURRENT_SCHEMA_VERSION, CompositionProfile, OperationEvent, OperationId, OperationKind,
    ProfileDocument, decode_profiles,
};

pub struct StudioApp {
    section: Section,
    workspace_root: String,
    workspace_root_draft: String,
    fallback_workspace_root: PathBuf,
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
    starter_workspace_path: String,
    starter_workspace_organization: String,
    starter_workspace_name: String,
    new_workspace_file_path: String,
    unregistered_projects: vapor_studio_core::UnregisteredProjectScan,
    pending_content_unregistration: Option<vapor_studio_core::SourceContent>,
    registered_sources: Vec<PathBuf>,
    active_source: Option<PathBuf>,
    contents: Vec<vapor_studio_core::SourceContent>,
    catalog: Vec<vapor_studio_core::CatalogContent>,
    catalog_source_filter: CatalogSourceFilter,
    catalog_kind_filter: Option<String>,
    selected_catalog_content: Option<String>,
    workshop_preview: Option<vapor_studio_core::WorkshopPackagePreview>,
    workshop_id: String,
    workshop_account: String,
    workshop_change_note: String,
    workshop_confirmation: String,
    pending_workshop: Option<vapor_studio_core::WorkshopAcquisition>,
    workshop_search_text: String,
    workshop_results: Vec<vapor_studio_core::WorkshopItem>,
    workshop_results_query: Option<String>,
    workshop_subscriptions: std::collections::BTreeSet<String>,
    workshop_subscription_details:
        std::collections::BTreeMap<String, vapor_studio_core::WorkshopSubscription>,
    workshop_result_total: u32,
    workshop_page: u32,
    composition_preview: Option<vapor_studio_core::CompositionPreview>,
    workspace_files: Vec<PathBuf>,
    selected_workspace_file: Option<PathBuf>,
    workspace_editor: String,
    workspace_editor_original: String,
    workspace_editor_toml_diagnostic: Option<Result<(), String>>,
    workspace_editor_dirty: bool,
    workspace_files_open: bool,
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
            workspace_root,
            workspace_root_draft,
            fallback_workspace_root: root,
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
            workshop_preview: None,
            workshop_id: String::new(),
            workshop_account: String::new(),
            workshop_change_note: String::new(),
            workshop_confirmation: String::new(),
            pending_workshop: None,
            workshop_search_text: String::new(),
            workshop_results: Vec::new(),
            workshop_results_query: None,
            workshop_subscriptions: std::collections::BTreeSet::new(),
            workshop_subscription_details: std::collections::BTreeMap::new(),
            workshop_result_total: 0,
            workshop_page: 1,
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
            workspace_files: Vec::new(),
            selected_workspace_file: None,
            workspace_editor: String::new(),
            workspace_editor_original: String::new(),
            workspace_editor_toml_diagnostic: None,
            workspace_editor_dirty: false,
            workspace_files_open: false,
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
            starter_workspace_path: String::new(),
            starter_workspace_organization: "my-workshop".to_owned(),
            starter_workspace_name: "my-game".to_owned(),
            new_workspace_file_path: String::new(),
            unregistered_projects: vapor_studio_core::UnregisteredProjectScan::default(),
            pending_content_unregistration: None,
            registered_sources: Vec::new(),
            active_source: None,
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

    fn draw_navigation(&mut self, ui: &mut egui::Ui) {
        ui.heading("VAPOR");
        ui.weak("Studio");
        ui.add_space(14.0);
        for section in Section::ALL {
            if ui
                .selectable_label(self.section == section, section.label())
                .clicked()
            {
                self.section = section;
                if section == Section::Workspace && self.workspace_files.is_empty() && !self.running
                {
                    self.refresh_workspace_files();
                }
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            if ui
                .selectable_label(self.section == Section::Diagnostics, "Diagnostics")
                .clicked()
            {
                self.section = Section::Diagnostics;
            }
            ui.separator();
            ui.small("Vapor Studio");
        });
    }

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let previous_profile = self.active_profile;
            ui.heading(self.section.label());
            ui.separator();
            ui.label("Profile");
            egui::ComboBox::from_id_salt("active-profile")
                .selected_text(&self.profiles[self.active_profile].name)
                .show_ui(ui, |ui| {
                    for (index, profile) in self.profiles.iter().enumerate() {
                        ui.selectable_value(&mut self.active_profile, index, &profile.name);
                    }
                });
            if self.active_profile != previous_profile {
                let requested_profile = self.active_profile;
                self.active_profile = previous_profile;
                self.select_profile(requested_profile);
            }
            ui.add(egui::TextEdit::singleline(&mut self.search_query).hint_text("Search content"))
                .on_hover_text("Search the current catalog");
            if ui
                .add_enabled(
                    !self.running && !self.workspace_editor_dirty,
                    egui::Button::new("+ Profile"),
                )
                .clicked()
            {
                self.new_profile_name = format!("Composition {}", self.profiles.len() + 1);
                self.add_profile();
            }
            if self.running {
                ui.separator();
                ui.spinner();
                ui.label("Working…");
            }
        });
    }

    fn preview_steam_package(&mut self) {
        self.spawn(OperationKind::StageRootDistribution, |_context| {
            let result = vapor_sdk_core::root::package::root_package(
                &vapor_sdk_core::root::types::RootPackageRequest {
                    plan: true,
                    app_id: vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                    depot_id: vapor_sdk_core::root::types::ROOT_STEAM_DEPOT_ID,
                    description: "Vapor Studio package preview".to_owned(),
                    set_live: None,
                },
            )
            .map(|report| {
                format!(
                    "Steam root preview · App {} · depot {} · {} source roots · {} files · output {}",
                    report.app_id,
                    report.depot_id,
                    report.included_roots.len(),
                    report.copied_files,
                    report.package_root.display(),
                )
            })
            .map_err(|error| error.to_string());
            OperationResult::Message(result)
        });
        self.status = "Preparing a read-only Steam root package plan…".to_owned();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_operation();
        egui::Panel::left("studio-navigation")
            .resizable(false)
            .default_size(190.0)
            .show(ui, |ui| self.draw_navigation(ui));
        egui::Panel::top("studio-header").show(ui, |ui| self.draw_header(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            match self.section {
                Section::Home => self.draw_home(ui),
                Section::Discover => self.draw_discover(ui),
                Section::Library => self.draw_library(ui),
                Section::Compose => self.draw_composition(ui),
                Section::Create => self.draw_create(ui),
                Section::Workspace => self.draw_workspace(ui),
                Section::Diagnostics => {
                    self.diagnostics.show(ui);
                    egui::CollapsingHeader::new("Steam root package tools").show(ui, |ui| {
                        ui.label("Root app/depot publishing is separate from Workshop content publishing.");
                        if ui.add_enabled(!self.running, egui::Button::new("Preview root package")).clicked() {
                            self.preview_steam_package();
                        }
                    });
                }
            }
            self.draw_operation_center(ui);
            if let Some(recovery) = &self.profile_recovery {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, recovery);
            }
        });
        if self.running {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("workspace_root", self.workspace_root.clone());
        let document = ProfileDocument {
            schema_version: CURRENT_SCHEMA_VERSION,
            profiles: self.profiles.clone(),
        };
        if let Ok(encoded) = serde_json::to_string(&document) {
            storage.set_string("profiles", encoded);
        }
        if let Some(original) = &self.profile_recovery_backup {
            storage.set_string("profiles.recovery_backup", original.clone());
        }
        storage.set_string("active_profile", self.active_profile.to_string());
        if let Some(profile) = self.profiles.get(self.active_profile) {
            storage.set_string("active_profile_id", profile.id.clone());
        }
    }
}
