#[path = "components/mod.rs"]
mod components;
#[path = "operation_center.rs"]
mod operation_center;
#[path = "operations.rs"]
mod operations;
#[path = "profiles.rs"]
mod profiles;
#[path = "pages/mod.rs"]
mod pages;
#[path = "workspace_files.rs"]
mod workspace_files;
#[path = "workspace_ops.rs"]
mod workspace_ops;

use crate::navigation::{CatalogSourceFilter, Section};
use eframe::egui;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc::Receiver};
use operations::OperationResult;
use vapor_core::{ContentKind, OperationEvent as CoreOperationEvent};
use vapor_studio_core::{
    CURRENT_SCHEMA_VERSION, CompositionProfile, ContentIdentity, OperationEvent, OperationId,
    OperationKind, ProfileDocument, decode_profiles,
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

    fn install_selected(&mut self) {
        if !self.require_saved_workspace("install this workspace composition") {
            return;
        }
        let Some(profile) = self.profiles.get(self.active_profile).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let workspace = profile.workspace.map(|workspace| workspace.root);
        let executable = match std::env::current_exe() {
            Ok(executable) => executable,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = root_package
            .version
            .as_deref()
            .map(|version| format!("{}@{}", root_package.id, version))
            .unwrap_or_else(|| root_package.id.clone());
        self.spawn(OperationKind::Install, move |_context| {
            OperationResult::Install(vapor_studio_core::install_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
            ))
        });
        self.status = "Installing the selected composition and dependencies…".to_owned();
    }

    fn update_selected(&mut self) {
        if !self.require_saved_workspace("update this workspace composition") {
            return;
        }
        let Some(profile) = self.profiles.get(self.active_profile).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let workspace = profile.workspace.map(|workspace| workspace.root);
        let executable = match std::env::current_exe() {
            Ok(executable) => executable,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        self.spawn(OperationKind::Update, move |_context| {
            OperationResult::Update(vapor_studio_core::update_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
            ))
        });
        self.status = "Updating the selected composition through Vapor Shell…".to_owned();
    }

    pub(super) fn preview_selected_composition(&mut self) {
        if !self.require_saved_workspace("resolve this workspace composition") {
            return;
        }
        let Some(profile) = self.profiles.get(self.active_profile).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let selector = content_selector(&root_package);
        let workspace = profile.workspace.map(|workspace| workspace.root);
        self.preview_composition(selector, workspace, "Resolving this profile against workspace, cache, and installed content…");
    }

    pub(super) fn preview_source_composition(&mut self, selector: String) {
        if !self.require_open_workspace("preview this source composition") {
            return;
        }
        let workspace = Some(PathBuf::from(self.workspace_root.trim()));
        self.preview_composition(selector, workspace, "Resolving this source composition against workspace, cache, and installed content…");
    }

    fn preview_composition(
        &mut self,
        selector: String,
        workspace: Option<PathBuf>,
        status: &str,
    ) {
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.composition_preview = None;
        self.spawn(OperationKind::Resolve, move |_context| {
            OperationResult::CompositionPreview(vapor_studio_core::preview_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
            ))
        });
        self.status = status.to_owned();
    }

    fn catalog_identity_is_conflicted(&self, selector: &str) -> bool {
        self.catalog.iter().any(|content| {
            catalog_content_selector(content) == selector && !content.is_unambiguous()
        })
    }

    fn invalidate_workshop_review(&mut self) {
        self.workshop_preview = None;
        self.workshop_confirmation.clear();
    }

    pub(super) fn stage_workshop_content(&mut self, selector: String) {
        if self.catalog_identity_is_conflicted(&selector) {
            self.status = format!(
                "Cannot stage {selector}: local records for this exact version disagree. Review the catalog conflict first."
            );
            return;
        }
        if !self.require_open_workspace("stage this Workshop item") {
            return;
        }
        self.invalidate_workshop_review();
        let workspace = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        self.spawn(OperationKind::StageWorkshop, move |_context| {
            OperationResult::WorkshopStage(vapor_studio_core::stage_workshop_preview(
                &executable,
                &workspace,
                &selector,
            ))
        });
        self.status = "Building and staging a Workshop review package…".to_owned();
    }

    pub(super) fn publish_reviewed_workshop_content(
        &mut self,
        selector: String,
        expected_review: vapor_studio_core::WorkshopPackagePreview,
    ) {
        if self.catalog_identity_is_conflicted(&selector) {
            self.status = format!(
                "Cannot publish {selector}: local records for this exact version disagree. Review the catalog conflict first."
            );
            return;
        }
        if !self.require_open_workspace("publish this Workshop item") {
            return;
        }
        let account = self.workshop_account.trim().to_owned();
        if account.is_empty() {
            self.status =
                "Enter the Steam publisher account used by the local SteamCMD provider.".to_owned();
            return;
        }
        let expected_confirmation = format!("PUBLISH {selector}");
        if self.workshop_confirmation.trim() != expected_confirmation {
            self.status =
                format!("Type {expected_confirmation} exactly to authorize this Workshop upload.");
            return;
        }
        let workspace = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        let change_note = self.workshop_change_note.trim().to_owned();
        self.spawn(OperationKind::PublishWorkshop, move |_context| {
            OperationResult::WorkshopPublish(vapor_studio_core::publish_reviewed_workshop_content(
                &executable,
                &workspace,
                &selector,
                &expected_review,
                &account,
                (!change_note.is_empty()).then_some(change_note.as_str()),
            ))
        });
        self.status = "Uploading the reviewed Workshop artifact through Vapor Shell…".to_owned();
    }

    pub(super) fn download_workshop_item(&mut self) {
        let published_file_id = self.workshop_id.trim().to_owned();
        if published_file_id.is_empty() {
            self.status = "Enter a numeric Steam Workshop item ID.".to_owned();
            return;
        }
        if !published_file_id.bytes().all(|byte| byte.is_ascii_digit()) {
            self.status =
                "Workshop downloads currently require a numeric PublishedFileId.".to_owned();
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Acquire, move |_context| {
            OperationResult::WorkshopAcquisition(vapor_studio_core::download_workshop_item(
                &executable,
                &published_file_id,
            ))
        });
        self.status = "Downloading the public Workshop item into Vapor's cache…".to_owned();
    }

    fn verify_selected_installation(&mut self) {
        let Some(root_package) = self
            .profiles
            .get(self.active_profile)
            .and_then(|profile| profile.root_package.clone())
        else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        let selectors = self.profiles[self.active_profile]
            .resolved_graph
            .as_ref()
            .map(|graph| {
                graph
                    .nodes
                    .iter()
                    .map(|node| content_selector(&node.content))
                    .collect()
            })
            .unwrap_or_else(|| vec![content_selector(&root_package)]);
        self.spawn(OperationKind::Verify, move |_context| {
            OperationResult::Verify(vapor_studio_core::verify_installation(
                &executable,
                &content_selector(&root_package),
                &selectors,
            ))
        });
        self.status = "Verifying installed composition…".to_owned();
    }

    fn repair_selected_installation(&mut self) {
        if !self.require_saved_workspace("repair this workspace composition") {
            return;
        }
        let Some(root_package) = self
            .profiles
            .get(self.active_profile)
            .and_then(|profile| profile.root_package.clone())
        else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        let workspace = self.profiles[self.active_profile]
            .workspace
            .as_ref()
            .map(|workspace| workspace.root.clone());
        let selector = content_selector(&root_package);
        let graph_selectors = self.profiles[self.active_profile]
            .resolved_graph
            .as_ref()
            .map(|graph| {
                graph
                    .nodes
                    .iter()
                    .map(|node| content_selector(&node.content))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.spawn(OperationKind::Repair, move |_context| {
            OperationResult::Repair(vapor_studio_core::repair_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
                &graph_selectors,
            ))
        });
        self.status = "Repairing installed composition…".to_owned();
    }

    fn run_installed_selected(&mut self) {
        let Some(profile) = self.profiles.get(self.active_profile).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        let workspace = profile.workspace.map(|workspace| workspace.root);
        let arguments = profile.launch.arguments;
        self.spawn(OperationKind::RunInstalled, move |_context| {
            OperationResult::Message(vapor_studio_core::run_installed_profile(
                &executable,
                workspace.as_deref(),
                &selector,
                &arguments,
            ))
        });
        self.status = "Launching the installed composition for this profile…".to_owned();
    }

    fn launch_default_profile(&mut self) {
        if self.profiles[self.active_profile].root_package.is_none() {
            let executable = match std::env::current_exe() {
                Ok(value) => value,
                Err(error) => {
                    self.status = format!("Could not locate the Studio executable: {error}");
                    return;
                }
            };
            match vapor_studio_core::default_packagepack_selector(&executable, "default") {
                Ok(id) => {
                    self.profiles[self.active_profile].root_package =
                        Some(ContentIdentity { id, version: None })
                }
                Err(error) => {
                    self.status = format!("This app has no default playable composition: {error}");
                    return;
                }
            }
        }
        self.run_installed_selected();
    }

    fn run_selected(&mut self) {
        if !self.require_open_workspace("build and play this workspace composition") {
            return;
        }
        let Some(profile) = self.profiles.get(self.active_profile).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let Some(workspace) = profile.workspace.map(|workspace| workspace.root) else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        let arguments = profile.launch.arguments;
        self.spawn(OperationKind::Run, move |_context| {
            OperationResult::RunProfile(vapor_studio_core::run_source_profile(
                &executable,
                &workspace,
                &selector,
                &arguments,
            ))
        });
        self.status =
            "Installing source changes and launching this profile through Vapor Shell…".to_owned();
    }

    pub(in crate::app) fn search_workshop(&mut self) {
        let query = self.workshop_search_text.trim().to_owned();
        let page = self.workshop_page;
        self.workshop_results.clear();
        self.workshop_results_query = None;
        self.workshop_result_total = 0;
        self.spawn(OperationKind::SearchWorkshop, move |_context| {
            OperationResult::WorkshopSearch(
                vapor_studio_core::search_workshop(
                    vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                    &query,
                    page,
                ),
                query,
            )
        });
        self.status = "Searching Steam Workshop…".to_owned();
    }

    pub(in crate::app) fn refresh_workshop_subscriptions(&mut self) {
        self.spawn(OperationKind::LoadWorkshopSubscriptions, move |_context| {
            OperationResult::WorkshopSubscriptions(vapor_studio_core::workshop_subscriptions(
                vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
            ))
        });
        self.status = "Loading Steam Workshop subscriptions…".to_owned();
    }

    pub(in crate::app) fn set_workshop_subscription(&mut self, id: String, subscribed: bool) {
        let parsed_id = match id.parse::<u64>() {
            Ok(id) if id != 0 => id,
            _ => {
                self.status = "Steam returned an invalid PublishedFileId.".to_owned();
                return;
            }
        };
        self.spawn(OperationKind::ChangeWorkshopSubscription, move |_context| {
            OperationResult::WorkshopSubscriptionChange(
                if subscribed {
                    vapor_studio_core::subscribe_workshop_item(
                        vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                        parsed_id,
                    )
                } else {
                    vapor_studio_core::unsubscribe_workshop_item(
                        vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                        parsed_id,
                    )
                },
                id,
                subscribed,
            )
        });
        self.status = if subscribed {
            "Subscribing to Steam Workshop item…"
        } else {
            "Unsubscribing from Steam Workshop item…"
        }
        .to_owned();
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

    pub(super) fn load_source_composition(&mut self) {
        if !self.require_open_workspace("edit a source content dependency graph") { return; }
        let selector = self.composition_edit_selector.trim().to_owned();
        if selector.is_empty() { self.status = "Select a source content project first.".to_owned(); return; }
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() { Ok(value) => value, Err(error) => { self.status = format!("Could not locate the Studio executable: {error}"); return; } };
        self.spawn(OperationKind::LoadComposition, move |_context| {
            OperationResult::SourceCompositionLoaded(vapor_studio_core::load_source_composition(&executable, &root, &selector))
        });
        self.status = format!("Loading dependencies for {selector} through Vapor Shell…");
    }

    pub(super) fn save_source_composition(&mut self) {
        if !self.require_open_workspace("save a source content dependency graph") { return; }
        let Some(composition) = self.composition_edit.clone() else { self.status = "Load a source content project before saving.".to_owned(); return; };
        let selector = composition.version.as_deref()
            .map(|version| format!("{}@{version}", composition.id))
            .unwrap_or_else(|| composition.id.clone());
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() { Ok(value) => value, Err(error) => { self.status = format!("Could not locate the Studio executable: {error}"); return; } };
        self.spawn(OperationKind::SaveComposition, move |_context| {
            OperationResult::SourceCompositionSaved(vapor_studio_core::save_source_composition(
                &executable, &root, &selector, &composition.source, &composition.links,
            ))
        });
        self.status = "Validating and saving the source content graph through Vapor Shell…".to_owned();
    }

    pub(super) fn load_source_workshop_metadata(&mut self, content: &vapor_studio_core::SourceContent) {
        if !self.require_open_workspace("edit Workshop metadata") { return; }
        let selector = source_content_selector(content);
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => { self.status = format!("Could not locate the Studio executable: {error}"); return; }
        };
        self.spawn(OperationKind::LoadWorkshopMetadata, move |_context| {
            OperationResult::SourceWorkshopLoaded(vapor_studio_core::load_source_workshop_metadata(
                &executable, &root, &selector,
            ))
        });
        self.status = format!("Loading Workshop metadata for {selector} through Vapor Shell…");
    }

    pub(super) fn load_source_runtime_outputs(&mut self, content: &vapor_studio_core::SourceContent) {
        if !self.require_open_workspace("edit runtime output declarations") { return; }
        let selector = source_content_selector(content);
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => { self.status = format!("Could not locate the Studio executable: {error}"); return; }
        };
        self.spawn(OperationKind::LoadRuntimeOutputs, move |_context| {
            OperationResult::SourceRuntimeOutputsLoaded(vapor_studio_core::load_source_runtime_outputs(
                &executable, &root, &selector,
            ))
        });
        self.status = format!("Loading runtime output declarations for {selector} through Vapor Shell…");
    }

    pub(super) fn save_source_runtime_outputs(&mut self) {
        if !self.require_open_workspace("save runtime output declarations") { return; }
        if self.workspace_editor_dirty {
            self.status = "Save or reload the open file before saving runtime outputs.".to_owned();
            return;
        }
        let Some(outputs) = self.runtime_outputs_edit.clone() else {
            self.status = "Load a source content project's runtime outputs first.".to_owned();
            return;
        };
        let parse_names = |text: &str| text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let binaries = parse_names(&self.runtime_binaries_text);
        let libraries = parse_names(&self.runtime_libraries_text);
        let selector = outputs.version.as_deref()
            .map(|version| format!("{}@{version}", outputs.id))
            .unwrap_or_else(|| outputs.id.clone());
        let expected_source = outputs.source;
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => { self.status = format!("Could not locate the Studio executable: {error}"); return; }
        };
        self.spawn(OperationKind::SaveRuntimeOutputs, move |_context| {
            OperationResult::SourceRuntimeOutputsSaved(vapor_studio_core::save_source_runtime_outputs(
                &executable, &root, &selector, &expected_source, &binaries, &libraries,
            ))
        });
        self.status = format!("Validating and saving runtime outputs for {selector} through Vapor Shell…");
    }

    fn set_runtime_output_fields(&mut self, outputs: &vapor_studio_core::SourceRuntimeOutputs) {
        self.runtime_binaries_text = outputs.binaries.join("\n");
        self.runtime_libraries_text = outputs.libraries.join("\n");
    }

    fn invalidate_profiles_for_source_content(&mut self, identity: &ContentIdentity) {
        self.invalidate_workshop_review();
        for (index, profile) in self.profiles.iter_mut().enumerate() {
            let affected = profile.root_package.as_ref().is_some_and(|root| root == identity)
                || profile.resolved_graph.as_ref()
                    .is_some_and(|graph| graph.nodes.iter().any(|node| &node.content == identity));
            if affected {
                profile.resolved_graph = None;
                profile.installation = vapor_studio_core::InstallationState::NotInstalled;
                self.repreview_after_catalog_refresh |= index == self.active_profile;
            }
        }
    }

    pub(super) fn save_source_workshop_metadata(&mut self) {
        if !self.require_open_workspace("save Workshop metadata") { return; }
        if self.workspace_editor_dirty {
            self.status = "Save or reload the open file before saving Workshop metadata.".to_owned();
            return;
        }
        let Some(metadata) = self.workshop_metadata_edit.clone() else {
            self.status = "Load a source content project's Workshop metadata first.".to_owned();
            return;
        };
        let app_id = match self.workshop_metadata_app_id.trim() {
            "" => None,
            value => match value.parse::<u32>() {
                Ok(value) if value > 0 => Some(value),
                _ => { self.status = "Steam AppID must be a positive number.".to_owned(); return; }
            },
        };
        let non_empty = |value: &str| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        };
        let policy = vapor_studio_core::WorkshopPolicyDraft {
            app_id,
            published_file_id: non_empty(&self.workshop_metadata_published_file_id),
            visibility: non_empty(&self.workshop_metadata_visibility),
            title: non_empty(&self.workshop_metadata_title),
            description: non_empty(&self.workshop_metadata_description),
            tags: self.workshop_metadata_tags.split(',').map(str::trim)
                .filter(|tag| !tag.is_empty()).map(ToOwned::to_owned).collect(),
            change_note: non_empty(&self.workshop_metadata_change_note),
        };
        let selector = metadata.version.as_deref()
            .map(|version| format!("{}@{version}", metadata.id))
            .unwrap_or_else(|| metadata.id.clone());
        let expected_source = metadata.source;
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => { self.status = format!("Could not locate the Studio executable: {error}"); return; }
        };
        self.spawn(OperationKind::SaveWorkshopMetadata, move |_context| {
            OperationResult::SourceWorkshopSaved(vapor_studio_core::save_source_workshop_metadata(
                &executable, &root, &selector, &expected_source, &policy,
            ))
        });
        self.status = format!("Validating and saving Workshop metadata for {selector} through Vapor Shell…");
    }

    fn set_workshop_metadata_fields(&mut self, policy: &vapor_studio_core::WorkshopPolicyDraft) {
        self.workshop_metadata_app_id = policy.app_id.map(|value| value.to_string()).unwrap_or_default();
        self.workshop_metadata_published_file_id = policy.published_file_id.clone().unwrap_or_default();
        self.workshop_metadata_visibility = policy.visibility.clone().unwrap_or_default();
        self.workshop_metadata_title = policy.title.clone().unwrap_or_default();
        self.workshop_metadata_description = policy.description.clone().unwrap_or_default();
        self.workshop_metadata_tags = policy.tags.join(", ");
        self.workshop_metadata_change_note = policy.change_note.clone().unwrap_or_default();
    }

    pub(super) fn use_edited_composition_for_profile(&mut self) {
        let Some(composition) = self.composition_edit.as_ref() else { self.status = "Load a source Packagepack first.".to_owned(); return; };
        if composition.kind != vapor_core::ContentKind::Packagepack.as_str() {
            self.status = "Only a Packagepack can be selected as a launch profile root.".to_owned();
            return;
        }
        let root = ContentIdentity { id: composition.id.clone(), version: composition.version.clone() };
        self.assign_profile_root(root, Some(PathBuf::from(self.workspace_root.trim())));
        self.preview_selected_composition();
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
