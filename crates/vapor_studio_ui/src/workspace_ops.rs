use super::{StudioApp, content_selector, source_content_selector};
use super::operations::{OperationResult, discover_workspace_and_catalog};
use super::workspace_files::{
    create_new_workspace_file, discover_workspace_files, read_workspace_file, write_workspace_file,
};
use crate::navigation::Section;
use std::{fs, path::PathBuf};
use vapor_core::{DevelopmentOperation, VaporWorkspace};
use vapor_studio_core::{OperationKind, WorkspaceAssociation};

impl StudioApp {
    pub(super) fn run_operation(&mut self, operation: DevelopmentOperation) {
        if !self.require_open_workspace("run a workspace operation") {
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        let kind = match operation {
            DevelopmentOperation::Fmt => OperationKind::Fmt,
            DevelopmentOperation::Check => OperationKind::Check,
            DevelopmentOperation::Test => OperationKind::Test,
            DevelopmentOperation::Build => OperationKind::Build,
            DevelopmentOperation::Clean => OperationKind::Clean,
        };
        self.spawn(kind, move |context| {
            OperationResult::Workspace(
                vapor_core::run_workspace_operation_at_with_context(&root, operation, &context)
                    .map_err(|error| error.to_string()),
            )
        });
        self.status = format!("Running Vapor-managed {operation}…");
    }

    pub(super) fn discover(&mut self) {
        if !self.require_saved_workspace("refresh workspace content") {
            return;
        }
        self.invalidate_workshop_review();
        let root = self
            .profiles
            .get(self.active_profile)
            .and_then(|profile| profile.workspace.as_ref())
            .map(|workspace| workspace.root.clone());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Discover, move |_context| {
            let (source, catalog) = discover_workspace_and_catalog(&executable, root.as_deref());
            OperationResult::Discovery(source, catalog)
        });
        self.status = "Refreshing the local content catalog through Vapor Shell…".to_owned();
    }

    pub(super) fn activate_workspace(
        &mut self,
        root: PathBuf,
        contents: Vec<vapor_studio_core::SourceContent>,
        catalog: Result<Vec<vapor_studio_core::CatalogContent>, String>,
        files: Vec<PathBuf>,
    ) {
        let workspace_changed = self.profiles[self.active_profile]
            .workspace
            .as_ref()
            .is_none_or(|workspace| workspace.root != root);
        let catalog_error = catalog.as_ref().err().cloned();
        self.workspace_root = root.display().to_string();
        self.workspace_root_draft = self.workspace_root.clone();
        self.draft_dependencies.clear();
        let profile = &mut self.profiles[self.active_profile];
        profile.workspace = Some(WorkspaceAssociation { root });
        if workspace_changed {
            profile.root_package = None;
            profile.resolved_graph = None;
            profile.installation = vapor_studio_core::InstallationState::Unknown;
        }
        self.contents = contents;
        self.catalog = catalog.unwrap_or_default();
        self.selected_catalog_content = None;
        self.composition_preview = None;
        self.composition_edit = None;
        self.composition_original_links = None;
        self.composition_edit_selector.clear();
        self.composition_link_candidate.clear();
        self.repreview_after_catalog_refresh = false;
        self.invalidate_workshop_review();
        self.workspace_files = files;
        self.selected_workspace_file = None;
        self.workspace_editor.clear();
        self.workspace_editor_original.clear();
        self.workspace_editor_toml_diagnostic = None;
        self.workspace_editor_dirty = false;
        self.workspace_files_open = false;
        self.new_workspace_file_path.clear();
        self.unregistered_projects = vapor_studio_core::UnregisteredProjectScan::default();
        self.pending_content_unregistration = None;
        self.section = Section::Workspace;
        self.status = catalog_error.map_or_else(
            || format!("Opened workspace {} · {} source items · {} local catalog entries.", self.workspace_root, self.contents.len(), self.catalog.len()),
            |error| format!("Opened workspace {} · {} source items. Local cache/install catalog could not load: {error}", self.workspace_root, self.contents.len()),
        );
    }

    pub(super) fn open_workspace_path(&mut self, path: PathBuf) {
        if self.manifest_metadata_dirty {
            self.workspace_root_draft = self.workspace_root.clone();
            self.status = "Save or discard the structured manifest edits before changing workspaces.".to_owned();
            return;
        }
        if self.workspace_editor_dirty {
            self.workspace_root_draft = self.workspace_root.clone();
            self.status =
                "Save or discard the current editor changes before changing workspaces.".to_owned();
            return;
        }
        if path.as_os_str().is_empty() {
            self.status = "Enter a workspace folder path.".to_owned();
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selected = path.clone();
        self.spawn(OperationKind::OpenWorkspace, move |_context| {
            let result = fs::canonicalize(&selected)
                .map_err(|error| format!("{}: {error}", selected.display()))
                .and_then(|root| {
                    VaporWorkspace::load(&root)
                        .map_err(|error| format!("{} is not a valid Vapor Workspace: {error}", root.display()))?;
                    let contents = vapor_studio_core::discover_source_content(&executable, &root)?;
                    let catalog =
                        vapor_studio_core::discover_content_catalog(&executable, Some(&root));
                    discover_workspace_files(&root).map(|files| (root, contents, catalog, files))
                });
            OperationResult::WorkspaceSelection(result)
        });
        self.status = format!("Opening workspace {}…", path.display());
    }

    pub(super) fn validate_source_project(&mut self, content: &vapor_studio_core::SourceContent) {
        if !self.require_open_workspace("validate this content project") {
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        let selector = source_content_selector(content);
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Validate, move |_context| {
            OperationResult::Message(vapor_studio_core::validate_profile_source(
                &executable, &root, &selector,
            ))
        });
        self.status = format!("Validating {selector} through Vapor Shell…");
    }

    pub(super) fn validate_selected_source(&mut self) {
        if !self.require_open_workspace("validate source content") {
            return;
        }
        let Some(root_package) = self
            .profiles
            .get(self.active_profile)
            .and_then(|profile| profile.root_package.clone())
        else {
            return;
        };
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        self.spawn(OperationKind::Validate, move |_context| {
            OperationResult::Message(vapor_studio_core::validate_profile_source(
                &executable,
                &root,
                &selector,
            ))
        });
        self.status = "Validating the selected source through Vapor Shell…".to_owned();
    }

    pub(super) fn validate_workspace_source(&mut self) {
        if !self.require_open_workspace("validate workspace content") {
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Validate, move |_context| {
            OperationResult::Message(vapor_studio_core::validate_workspace_source(
                &executable,
                &root,
            ))
        });
        self.status = "Validating authored content through Vapor…".to_owned();
    }

    pub(super) fn require_saved_workspace(&mut self, action: &str) -> bool {
        if self.manifest_metadata_dirty {
            self.status = format!(
                "Save or discard structured manifest edits before you {action}.",
            );
            return false;
        }
        if !self.workspace_editor_dirty {
            return true;
        }
        self.status = format!(
            "Save or reload the open file before you {action}; Vapor operations read saved files from disk.",
        );
        false
    }

    pub(super) fn require_open_workspace(&mut self, action: &str) -> bool {
        if !self.require_saved_workspace(action) {
            return false;
        }
        if self
            .profiles
            .get(self.active_profile)
            .and_then(|profile| profile.workspace.as_ref())
            .is_some()
            && !self.workspace_root.trim().is_empty()
        {
            return true;
        }
        self.status = format!("Open a Vapor Workspace before you {action}.");
        false
    }

    pub(super) fn refresh_workspace_files(&mut self) {
        if !self.require_open_workspace("read workspace files") {
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(OperationKind::DiscoverWorkspace, move |_context| {
            OperationResult::WorkspaceFiles(discover_workspace_files(&root))
        });
        self.status = "Reading workspace files…".to_owned();
    }

    pub(super) fn refresh_workspace_editor_toml_diagnostic(&mut self) {
        self.workspace_editor_toml_diagnostic = self
            .selected_workspace_file
            .as_deref()
            .filter(|path| {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                name == "Vapor.toml" || name.ends_with(".vapor.toml") || name == "Cargo.toml"
            })
            .map(|_| {
                self.workspace_editor
                    .parse::<toml::Value>()
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            });
    }

    pub(super) fn open_workspace_file(&mut self, path: PathBuf) {
        if !self.require_open_workspace("open a workspace file") {
            return;
        }
        if self.manifest_metadata_dirty {
            self.status = "Save or discard structured manifest edits before opening another file.".to_owned();
            return;
        }
        if self.workspace_editor_dirty {
            self.status = "Save or discard the current editor changes before opening another file."
                .to_owned();
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(OperationKind::OpenWorkspaceFile, move |_context| {
            OperationResult::FileContents(
                read_workspace_file(&root, &path).map(|contents| (path, contents)),
            )
        });
        self.status = "Opening workspace file…".to_owned();
    }

    pub(super) fn save_workspace_file(&mut self) {
        if !self.require_open_workspace("save a workspace file") {
            return;
        }
        let Some(path) = self.selected_workspace_file.clone() else {
            return;
        };
        let root = PathBuf::from(self.workspace_root.trim());
        let expected_contents = self.workspace_editor_original.clone();
        let contents = self.workspace_editor.clone();
        self.spawn(OperationKind::SaveWorkspaceFile, move |_context| {
            OperationResult::FileSaved(
                write_workspace_file(&root, &path, &expected_contents, &contents).map(|()| path),
            )
        });
        self.status = "Saving workspace file…".to_owned();
    }

    pub(super) fn reload_workspace_file(&mut self) {
        if !self.require_open_workspace("reload a workspace file") {
            return;
        }
        let Some(path) = self.selected_workspace_file.clone() else {
            return;
        };
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(OperationKind::OpenWorkspaceFile, move |_context| {
            OperationResult::FileContents(
                read_workspace_file(&root, &path).map(|contents| (path, contents)),
            )
        });
        self.status = "Reloading workspace file from disk…".to_owned();
    }

    pub(super) fn create_workspace_file(&mut self) {
        if !self.require_open_workspace("create a workspace file") {
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        let relative = PathBuf::from(self.new_workspace_file_path.trim());
        if relative.as_os_str().is_empty() {
            self.status = "Enter a file path relative to the workspace root.".to_owned();
            return;
        }
        self.spawn(OperationKind::CreateWorkspaceFile, move |_context| {
            OperationResult::WorkspaceFileCreated(create_new_workspace_file(&root, &relative))
        });
        self.status = "Creating a new workspace file…".to_owned();
    }

    pub(super) fn create_content_workspace(&mut self) {
        if !self.require_open_workspace("create content in this workspace") {
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let id = self.draft_content_id.trim().to_owned();
        let version = self.draft_content_version.trim().to_owned();
        let selected = self.draft_dependencies.clone();
        let app_id = match self.draft_workshop_app_id.trim() {
            "" => None,
            value => match value.parse::<u32>() {
                Ok(value) if value > 0 => Some(value),
                _ => {
                    self.status = "Steam AppID must be a positive number.".to_owned();
                    return;
                }
            },
        };
        let non_empty = |value: &str| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        };
        let workshop = vapor_studio_core::WorkshopPolicyDraft {
            app_id,
            published_file_id: non_empty(&self.draft_workshop_published_file_id),
            visibility: non_empty(&self.draft_workshop_visibility),
            title: non_empty(&self.draft_workshop_title),
            description: non_empty(&self.draft_workshop_description),
            tags: self
                .draft_workshop_tags
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            change_note: non_empty(&self.draft_workshop_change_note),
        };
        let kind = self.draft_content_kind;
        self.spawn(OperationKind::CreateContent, move |_context| {
            let result = vapor_studio_core::create_content_workspace_with_workshop(
                &executable,
                &root,
                &id,
                &version,
                kind,
                &selected,
                &workshop,
            )
            .and_then(|output| {
                let workspace = fs::canonicalize(&root)
                    .map_err(|error| format!("{}: {error}", root.display()))?;
                let manifest = output;
                let relative = manifest
                    .strip_prefix(&workspace)
                    .map(PathBuf::from)
                    .map_err(|error| {
                        format!("created manifest is outside the workspace: {error}")
                    })?;
                let contents = fs::read_to_string(&manifest)
                    .map_err(|error| format!("{}: {error}", manifest.display()))?;
                let files = discover_workspace_files(&workspace)?;
                Ok((relative, contents, files))
            });
            OperationResult::ContentWorkspace(result)
        });
        self.status = "Creating content and opening its manifest…".to_owned();
    }

    pub(super) fn create_starter_workspace(&mut self) {
        if !self.require_saved_workspace("create and open a starter workspace") {
            return;
        }
        let destination = PathBuf::from(self.starter_workspace_path.trim());
        if destination.as_os_str().is_empty() {
            self.status = "Choose an empty folder for the new starter workspace.".to_owned();
            return;
        }
        let organization = self.starter_workspace_organization.trim().to_owned();
        let name = self.starter_workspace_name.trim().to_owned();
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let open_executable = executable.clone();
        self.spawn(OperationKind::CreateWorkspace, move |_context| {
            let result = vapor_studio_core::create_basic_content_workspace(
                &executable,
                &destination,
                &organization,
                &name,
            )
            .and_then(|root| {
                vapor_studio_core::discover_source_content(&open_executable, &root)
                    .and_then(|contents| {
                        let catalog = vapor_studio_core::discover_content_catalog(
                            &open_executable,
                            Some(&root),
                        );
                        discover_workspace_files(&root)
                            .map(|files| (root.clone(), contents, catalog, files))
                    })
                    .map_err(|error| {
                        format!(
                            "Created workspace at {}, but Studio could not open it: {error}",
                            root.display()
                        )
                    })
            });
            OperationResult::WorkspaceSelection(result)
        });
        self.status =
            "Creating the maintained Engine, Game, and Packagepack starter workspace…".to_owned();
    }

    pub(super) fn scan_unregistered_content_projects(&mut self) {
        if !self.require_open_workspace("scan for unregistered content") {
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let root = PathBuf::from(self.workspace_root.trim());
        self.spawn(OperationKind::DiscoverContentProjects, move |_context| {
            OperationResult::UnregisteredProjectScan(
                vapor_studio_core::scan_unregistered_content_projects(&executable, &root),
            )
        });
        self.status = "Finding unregistered content manifests in this Workspace…".to_owned();
    }

    pub(super) fn adopt_content_project(&mut self, project_path: PathBuf) {
        if !self.require_open_workspace("adopt a content project") {
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let root = PathBuf::from(self.workspace_root.trim());
        let display_path = project_path.display().to_string();
        self.spawn(OperationKind::AdoptContentProject, move |_context| {
            OperationResult::AdoptedContentProject(
                vapor_studio_core::adopt_workspace_content_project(
                    &executable, &root, &project_path,
                ),
            )
        });
        self.status = format!("Validating and adopting {display_path}…");
    }

    pub(super) fn unregister_content_project(&mut self) {
        if !self.require_open_workspace("unregister a content project") {
            return;
        }
        let Some(project) = self.pending_content_unregistration.clone() else {
            self.status = "Select a registered content project to unregister.".to_owned();
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let root = PathBuf::from(self.workspace_root.trim());
        let id = project.id.clone();
        let display = format!("{} · {}", project.id, project.root.display());
        self.spawn(OperationKind::UnregisterContentProject, move |_context| {
            OperationResult::UnregisteredContentProject(
                vapor_studio_core::unregister_workspace_content_project(&executable, &root, &id),
            )
        });
        self.status = format!("Removing {display} from Vapor's content registry…");
    }

    pub(super) fn remember_open_workspace(&mut self) {
        if !self.require_open_workspace("add this workspace to Vapor sources") {
            return;
        }
        let root = PathBuf::from(self.workspace_root.trim());
        if root.as_os_str().is_empty() {
            self.status = "Open a workspace before adding it to Vapor sources.".to_owned();
            return;
        }
        if self.registered_sources.iter().any(|source| source == &root) {
            self.status = "This workspace is already registered with Vapor.".to_owned();
            return;
        }
        self.spawn(OperationKind::RememberWorkspace, move |_context| {
            OperationResult::RememberedSources(vapor_studio_core::remember_workspace(&root))
        });
        self.status = "Adding the open workspace to this Vapor installation…".to_owned();
    }

    pub(super) fn refresh_registered_sources(&mut self) {
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

impl eframe::App for StudioApp {
}
