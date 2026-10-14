use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::{collections::BTreeMap, fs, io::Write};
use vapor_core::{
    DevelopmentOperation, LocalContent, ResolvedComposition, VaporId, VaporWorkspace,
    build_cargo_realization, discover_local_content, generate_local_cargo_realization,
    resolve_local_packagepack, run_cargo_realization, run_workspace_operation_at,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Library,
    Compose,
    Workspace,
    Workshop,
    Publish,
    Diagnostics,
}

impl Section {
    const ALL: [Self; 6] = [
        Self::Library,
        Self::Compose,
        Self::Workspace,
        Self::Workshop,
        Self::Publish,
        Self::Diagnostics,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Compose => "Compose",
            Self::Workspace => "Workspace",
            Self::Workshop => "Workshop",
            Self::Publish => "Publish",
            Self::Diagnostics => "Diagnostics",
        }
    }
}

enum OperationResult {
    Workspace(Result<VaporWorkspace, String>),
    Discovery(Result<Vec<LocalContent>, String>),
    Composition(Result<ResolvedComposition, String>),
    Message(Result<String, String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StudioProfile {
    name: String,
    selected_content: Option<String>,
    #[serde(default)]
    workspace_root: Option<PathBuf>,
}

pub struct StudioApp {
    section: Section,
    workspace_root: String,
    profiles: Vec<StudioProfile>,
    active_profile: usize,
    new_profile_name: String,
    draft_packagepack_id: String,
    draft_packagepack_version: String,
    draft_dependencies: Vec<String>,
    registered_sources: Vec<PathBuf>,
    active_source: Option<PathBuf>,
    contents: Vec<LocalContent>,
    status: String,
    running: bool,
    autoplay_pending: bool,
    receiver: Option<Receiver<OperationResult>>,
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
        let root = workspace
            .or_else(|| saved_workspace.map(PathBuf::from))
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let profiles = creation
            .storage
            .and_then(|storage| storage.get_string("profiles"))
            .and_then(|value| serde_json::from_str::<Vec<StudioProfile>>(&value).ok())
            .filter(|profiles| !profiles.is_empty())
            .unwrap_or_else(|| {
                vec![StudioProfile {
                    name: "Default".to_owned(),
                    selected_content: None,
                    workspace_root: None,
                }]
            });
        let active_profile = creation
            .storage
            .and_then(|storage| storage.get_string("active_profile"))
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| *index < profiles.len())
            .unwrap_or(0);
        let workspace_root = profiles[active_profile]
            .workspace_root
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| root.display().to_string());
        let mut app = Self {
            section: Section::Library,
            workspace_root,
            profiles,
            active_profile,
            new_profile_name: String::new(),
            contents: Vec::new(),
            draft_packagepack_id: String::new(),
            draft_packagepack_version: "0.1.0".to_owned(),
            draft_dependencies: Vec::new(),
            registered_sources: Vec::new(),
            active_source: None,
            status: "Choose a workspace or discover content in this folder.".to_owned(),
            running: false,
            autoplay_pending: false,
            receiver: None,
            diagnostics: vapor_devtools::connect(None),
        };
        app.refresh_registered_sources();
        if play_mode {
            app.autoplay_pending = true;
            app.discover();
        }
        app
    }

    fn poll_operation(&mut self) {
        let result = self
            .receiver
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok());
        if let Some(result) = result {
            self.receiver = None;
            self.running = false;
            match result {
                OperationResult::Workspace(Ok(workspace)) => {
                    self.status = format!(
                        "Completed for {}/{} · {} projects",
                        workspace.manifest.workspace.organization,
                        workspace.manifest.workspace.name,
                        workspace.projects.len()
                    )
                }
                OperationResult::Workspace(Err(error)) => self.status = error,
                OperationResult::Discovery(Err(error)) => {
                    self.autoplay_pending = false;
                    self.status = error;
                }
                OperationResult::Composition(Ok(composition)) => {
                    self.status = format!(
                        "Resolved {} · {} content versions",
                        composition.root,
                        composition.nodes.len()
                    )
                }
                OperationResult::Composition(Err(error)) | OperationResult::Message(Err(error)) => {
                    self.status = error
                }
                OperationResult::Message(Ok(message)) => self.status = message,
                OperationResult::Discovery(Ok(contents)) => {
                    self.status = format!("Discovered {} local content items.", contents.len());
                    self.contents = contents;
                    if self.selected_content().is_some_and(|id| {
                        !self
                            .contents
                            .iter()
                            .any(|content| content.manifest.content.id.as_str() == id)
                    }) {
                        self.profiles[self.active_profile].selected_content = None;
                    }
                    if self.autoplay_pending {
                        self.autoplay_pending = false;
                        if self.selected_content().is_some() {
                            self.run_selected();
                        } else {
                            self.status = "No composition is selected for this profile. Choose one in Studio, then use Play.".to_owned();
                        }
                    }
                }
            }
        }
    }

    fn run_operation(&mut self, operation: DevelopmentOperation) {
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(move || {
            OperationResult::Workspace(
                run_workspace_operation_at(&root, operation).map_err(|error| error.to_string()),
            )
        });
        self.status = format!("Running Vapor-managed {operation}…");
    }

    fn discover(&mut self) {
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(move || {
            OperationResult::Discovery(
                discover_local_content(&root)
                    .map(|catalog| catalog.iter().cloned().collect())
                    .map_err(|error| error.to_string()),
            )
        });
        self.status = "Discovering local Vapor content…".to_owned();
    }

    fn resolve_selected(&mut self) {
        let Some(content_id) = self.selected_content().map(str::to_owned) else {
            return;
        };
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(move || {
            let result = discover_local_content(&root)
                .map_err(|error| error.to_string())
                .and_then(|catalog| {
                    let id = content_id
                        .parse::<VaporId>()
                        .map_err(|error| error.to_string())?;
                    resolve_local_packagepack(&catalog, &id).map_err(|error| error.to_string())
                });
            OperationResult::Composition(result)
        });
        self.status = "Resolving the selected Packagepack graph…".to_owned();
    }

    fn run_selected(&mut self) {
        let Some(content_id) = self.selected_content().map(str::to_owned) else {
            return;
        };
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(move || {
            let result = (|| {
                let catalog = discover_local_content(&root).map_err(|error| error.to_string())?;
                let id = content_id
                    .parse::<VaporId>()
                    .map_err(|error| error.to_string())?;
                let composition =
                    resolve_local_packagepack(&catalog, &id).map_err(|error| error.to_string())?;
                let realization = generate_local_cargo_realization(&root, &catalog, &composition)
                    .map_err(|error| error.to_string())?;
                build_cargo_realization(&realization).map_err(|error| error.to_string())?;
                run_cargo_realization(&realization).map_err(|error| error.to_string())?;
                Ok(format!("Finished {}", composition.root))
            })();
            OperationResult::Message(result)
        });
        self.status =
            "Building and running the selected Packagepack with Vapor-managed tools…".to_owned();
    }

    fn spawn(&mut self, operation: impl FnOnce() -> OperationResult + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(operation());
        });
        self.receiver = Some(receiver);
        self.running = true;
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
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.small("Vapor Studio · local workspace mode")
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
                if self.running {
                    self.active_profile = previous_profile;
                } else {
                    if let Some(root) = &self.profiles[self.active_profile].workspace_root {
                        self.workspace_root = root.display().to_string();
                    }
                    self.contents.clear();
                    self.discover();
                }
            }
            ui.text_edit_singleline(&mut self.new_profile_name)
                .on_hover_text("Name a composition profile");
            if ui.button("+ Profile").clicked() {
                self.add_profile();
            }
            if self.running {
                ui.separator();
                ui.spinner();
                ui.label("Working…");
            }
        });
    }

    fn add_profile(&mut self) {
        let name = self.new_profile_name.trim();
        if name.is_empty()
            || self
                .profiles
                .iter()
                .any(|profile| profile.name.eq_ignore_ascii_case(name))
        {
            self.status = "Enter a new, non-empty profile name.".to_owned();
            return;
        }
        self.profiles.push(StudioProfile {
            name: name.to_owned(),
            selected_content: None,
            workspace_root: Some(PathBuf::from(self.workspace_root.trim())),
        });
        self.active_profile = self.profiles.len() - 1;
        self.new_profile_name.clear();
        self.status = format!(
            "Created profile {}.",
            self.profiles[self.active_profile].name
        );
    }

    fn selected_content(&self) -> Option<&str> {
        self.profiles
            .get(self.active_profile)?
            .selected_content
            .as_deref()
    }

    fn draw_library(&mut self, ui: &mut egui::Ui) {
        ui.heading("Your compositions");
        ui.label("Local, Workshop, and first-party content will share this library as their providers connect.");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.running, egui::Button::new("Discover local content"))
                .clicked()
            {
                self.discover();
            }
            if ui
                .add_enabled(
                    self.selected_content().is_some() && !self.running,
                    egui::Button::new("Resolve graph"),
                )
                .clicked()
            {
                self.resolve_selected();
            }
            if ui
                .add_enabled(
                    self.selected_content().is_some() && !self.running,
                    egui::Button::new("Build & Play"),
                )
                .clicked()
            {
                self.run_selected();
            }
        });
        ui.separator();
        self.draw_content_list(ui);
    }

    fn draw_compose(&mut self, ui: &mut egui::Ui) {
        ui.heading("Create a Packagepack");
        ui.label("Choose a stable identity and compose it from local Vapor content.");
        ui.horizontal(|ui| {
            ui.label("Content ID");
            ui.text_edit_singleline(&mut self.draft_packagepack_id);
        });
        ui.horizontal(|ui| {
            ui.label("Version");
            ui.text_edit_singleline(&mut self.draft_packagepack_version);
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.running,
                    egui::Button::new("Refresh available content"),
                )
                .clicked()
            {
                self.discover();
            }
            if ui
                .add_enabled(!self.running, egui::Button::new("Create Packagepack"))
                .clicked()
            {
                self.create_packagepack();
            }
        });
        ui.separator();
        ui.label("Packagepack contents");
        if self.contents.is_empty() {
            ui.weak("Discover local content to choose dependencies.");
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for content in &self.contents {
                let id = content.version_id().to_string();
                let allowed =
                    vapor_core::allowed_pack_children(vapor_core::ContentKind::Packagepack)
                        .is_some_and(|kinds| kinds.contains(&content.manifest.content.kind));
                if !allowed {
                    continue;
                }
                let mut included = self.draft_dependencies.iter().any(|value| value == &id);
                if ui
                    .checkbox(
                        &mut included,
                        format!(
                            "{} · {} · {}",
                            id, content.manifest.content.kind, content.manifest.content.version
                        ),
                    )
                    .changed()
                {
                    if included {
                        self.draft_dependencies.push(id.to_owned());
                    } else {
                        self.draft_dependencies.retain(|value| value != &id);
                    }
                }
            }
        });
        ui.weak("The manifest is created with an exclusive file create; an existing Vapor.toml is never overwritten.");
    }

    fn preview_steam_package(&mut self) {
        self.spawn(|| {
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

    fn create_packagepack(&mut self) {
        let root = PathBuf::from(self.workspace_root.trim());
        let id = self.draft_packagepack_id.trim().to_owned();
        let version = self.draft_packagepack_version.trim().to_owned();
        let selected = self.draft_dependencies.clone();
        let content = self.contents.clone();
        self.spawn(move || {
            let result = (|| {
                let id = id.parse::<VaporId>().map_err(|error| error.to_string())?;
                let version = semver::Version::parse(&version)
                    .map_err(|error| format!("invalid version: {error}"))?;
                let mut dependencies = BTreeMap::new();
                for (index, selected_id) in selected.into_iter().enumerate() {
                    let child = content
                        .iter()
                        .find(|entry| entry.version_id().to_string() == selected_id)
                        .ok_or_else(|| {
                            format!("selected content is no longer available: {selected_id}")
                        })?;
                    let allowed =
                        vapor_core::allowed_pack_children(vapor_core::ContentKind::Packagepack)
                            .is_some_and(|kinds| kinds.contains(&child.manifest.content.kind));
                    if !allowed {
                        return Err(format!(
                            "{} cannot be included in a Packagepack",
                            child.manifest.content.kind
                        ));
                    }
                    if child.manifest.content.id == id {
                        return Err("a Packagepack cannot depend on itself".to_owned());
                    }
                    let alias = format!("content-{index}");
                    let requirement =
                        semver::VersionReq::parse(&format!("={}", child.manifest.content.version))
                            .map_err(|error| error.to_string())?;
                    dependencies.insert(
                        alias,
                        vapor_core::DependencySpec {
                            id: child.manifest.content.id.clone(),
                            version: requirement,
                        },
                    );
                }
                let output = content_output_dir(&root, id.as_str());
                let typed_manifest = vapor_core::ContentManifest {
                    content: vapor_core::ContentHeader {
                        id,
                        version,
                        kind: vapor_core::ContentKind::Packagepack,
                    },
                    dependencies,
                };
                let manifest =
                    toml::to_string_pretty(&typed_manifest).map_err(|error| error.to_string())?;
                vapor_core::parse_content_manifest(&manifest).map_err(|error| error.to_string())?;
                fs::create_dir_all(&output)
                    .map_err(|error| format!("{}: {error}", output.display()))?;
                let manifest_path = output.join("Vapor.toml");
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&manifest_path)
                    .map_err(|error| {
                        format!("cannot create {}: {error}", manifest_path.display())
                    })?;
                file.write_all(manifest.as_bytes())
                    .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
                Ok(format!(
                    "Created Packagepack manifest at {}",
                    manifest_path.display()
                ))
            })();
            OperationResult::Message(result)
        });
        self.status = "Creating the Packagepack manifest…".to_owned();
    }

    fn draw_content_list(&mut self, ui: &mut egui::Ui) {
        if self.contents.is_empty() {
            ui.weak("No local items discovered yet.");
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for content in &self.contents {
                let id = content.manifest.content.id.as_str();
                if ui
                    .selectable_label(
                        self.selected_content() == Some(id),
                        format!("{} · {}", id, content.manifest.content.kind.as_str()),
                    )
                    .clicked()
                {
                    self.profiles[self.active_profile].selected_content = Some(id.to_owned());
                }
                ui.small(format!(
                    "{} · {}",
                    content.manifest.content.version,
                    content.root.display()
                ));
                ui.separator();
            }
        });
    }

    fn draw_workspace(&mut self, ui: &mut egui::Ui) {
        ui.heading("Workspace operations");
        ui.label("Operations use the workspace pin and Vapor-managed toolchain and cache.");
        ui.horizontal(|ui| {
            if ui.button("Refresh registered workspaces").clicked() {
                self.refresh_registered_sources();
            }
            if self.registered_sources.is_empty() {
                ui.weak("No workspaces loaded from this Vapor installation yet.");
            }
        });
        if !self.registered_sources.is_empty() {
            ui.label("Registered workspaces");
            for source in self.registered_sources.clone() {
                let active = self.active_source.as_ref() == Some(&source);
                let selected = PathBuf::from(self.workspace_root.trim()) == source;
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(selected, source.display().to_string())
                        .clicked()
                    {
                        self.workspace_root = source.display().to_string();
                        self.profiles[self.active_profile].workspace_root = Some(source.clone());
                        self.profiles[self.active_profile].selected_content = None;
                        self.contents.clear();
                        self.status = format!("Selected workspace {}.", source.display());
                    }
                    if active {
                        ui.weak("Vapor active source");
                    }
                });
            }
        }
        let workspace_root_changed = ui
            .horizontal(|ui| {
                ui.label("Workspace root");
                ui.text_edit_singleline(&mut self.workspace_root).changed()
            })
            .inner;
        if workspace_root_changed {
            self.profiles[self.active_profile].workspace_root =
                Some(PathBuf::from(self.workspace_root.trim()));
            self.profiles[self.active_profile].selected_content = None;
            self.contents.clear();
        }
        ui.horizontal_wrapped(|ui| {
            for (label, operation) in [
                ("Format", DevelopmentOperation::Fmt),
                ("Check", DevelopmentOperation::Check),
                ("Test", DevelopmentOperation::Test),
                ("Build", DevelopmentOperation::Build),
                ("Clean", DevelopmentOperation::Clean),
            ] {
                if ui
                    .add_enabled(!self.running, egui::Button::new(label))
                    .clicked()
                {
                    self.run_operation(operation);
                }
            }
        });
    }

    fn refresh_registered_sources(&mut self) {
        let result = vapor_core::VaporInstallation::discover()
            .map_err(|error| error.to_string())
            .and_then(|installation| {
                vapor_core::source_state(&installation)
                    .map(|state| (state.known, state.active))
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok((known, active)) => {
                self.registered_sources = known;
                self.active_source = active;
                self.status = format!(
                    "Loaded {} registered workspace{} from this Vapor installation.",
                    self.registered_sources.len(),
                    if self.registered_sources.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                );
            }
            Err(error) => self.status = format!("Could not read registered workspaces: {error}"),
        }
    }
}

fn content_output_dir(workspace_root: &std::path::Path, id: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    let slug = id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();
    workspace_root
        .join("content")
        .join(format!("{slug}-{:016x}", hasher.finish()))
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_operation();
        egui::Panel::left("studio-navigation")
            .resizable(false)
            .default_size(190.0)
            .show(ui, |ui| self.draw_navigation(ui));
        egui::Panel::top("studio-header").show(ui, |ui| self.draw_header(ui));
        egui::CentralPanel::default().show_inside(ui, |ui| {
            match self.section {
                Section::Library => self.draw_library(ui), Section::Compose => self.draw_compose(ui),
                Section::Workspace => self.draw_workspace(ui),
                Section::Workshop => {
                    ui.heading("Workshop");
                    ui.label("The Vapor-CLI provider can acquire items by PublishedFileId through SteamCMD.");
                    ui.label("Its app-root cache and role-specific manifests are not yet connected to Studio's local content graph.");
                    ui.weak("This view stays read-only until the provider and library share one content identity and provenance contract.");
                }
                Section::Publish => {
                    ui.heading("Publish");
                    ui.label("Review the real root package inputs before a SteamPipe operation.");
                    if ui.add_enabled(!self.running, egui::Button::new("Preview Steam root package")).clicked() {
                        self.preview_steam_package();
                    }
                    ui.separator();
                    ui.weak("This creates a read-only plan. SteamCMD upload and branch changes need a separately reviewed confirmation step.");
                }
                Section::Diagnostics => self.diagnostics.show(ui),
            }
            ui.separator(); ui.label(&self.status);
        });
        if self.running {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("workspace_root", self.workspace_root.clone());
        storage.set_string(
            "profiles",
            serde_json::to_string(&self.profiles).unwrap_or_else(|_| "[]".to_owned()),
        );
        storage.set_string("active_profile", self.active_profile.to_string());
    }
}
