use crate::{
    host::StudioHostContext,
    persistence::{StudioPersistentState, StudioStateStore},
};
use std::path::{Path, PathBuf};
use vapor_studio_core::{
    CatalogContent, SourceContent, add_profile, discover_content_catalog, discover_source_content,
    duplicate_profile, remove_profile, rename_profile, scan_unregistered_content_projects,
};
use vapor_studio_protocol::{
    AvailableAction, CatalogAction, CatalogConflict, CatalogEntry, CatalogLocation,
    CatalogLocationKind, CompositionProfile, ContentIdentity, DurableStateSnapshot, HostSummary,
    InstallationState, LegacyStorageSeed, ProfileAction, SessionAction, StudioAction, StudioError,
    StudioErrorCode, StudioRequest, StudioResponse, StudioResponseKind, StudioRevision,
    StudioSnapshot, WorkspaceAssociation, WorkspaceContentSnapshot, WorkspaceProjectCandidate,
    WorkspaceSourceProject, WorkspaceWorkshopPolicy,
};

const MAX_CATALOG_ITEMS: usize = 10_000;
const MAX_WORKSPACE_PROJECTS: usize = 10_000;
const MAX_WORKSPACE_DIAGNOSTICS: usize = 1_000;

#[derive(Debug, Clone)]
pub struct ServiceBootstrap {
    pub application_executable: PathBuf,
    pub workspace_root: Option<PathBuf>,
    pub active_profile_id: Option<String>,
    pub fixture_mode: bool,
}

pub struct StudioSession {
    bootstrap: ServiceBootstrap,
    revision: StudioRevision,
    active_profile_id: Option<String>,
    selected_content_id: Option<String>,
    catalog: Vec<CatalogEntry>,
    catalog_query: String,
    catalog_error: Option<String>,
    workspace_content: WorkspaceContentSnapshot,
    durable_state: StudioPersistentState,
    state_store: Option<StudioStateStore>,
    host_context: Option<StudioHostContext>,
}

impl StudioSession {
    pub fn new(bootstrap: ServiceBootstrap) -> Self {
        let mut durable_state = StudioPersistentState::default();
        if let Some(active_profile_id) = bootstrap.active_profile_id.as_ref()
            && durable_state
                .profiles
                .profiles
                .iter()
                .any(|profile| profile.id == *active_profile_id)
        {
            durable_state.active_profile_id = active_profile_id.clone();
        }
        durable_state.workspace_root = bootstrap
            .workspace_root
            .as_ref()
            .map(|root| root.display().to_string());
        Self::with_state(bootstrap, durable_state, None, None)
    }

    pub fn with_persistent_state(
        mut bootstrap: ServiceBootstrap,
        host_context: StudioHostContext,
        store: StudioStateStore,
        legacy_storage: Option<&LegacyStorageSeed>,
    ) -> Result<Self, String> {
        let mut loaded = store.load_or_import(legacy_storage)?;
        if let Some(workspace_root) = bootstrap.workspace_root.as_ref() {
            let workspace = Some(workspace_root.display().to_string());
            let mut profile_changed = loaded.state.workspace_root != workspace;
            loaded.state.workspace_root = workspace;
            if let Some(profile) = loaded
                .state
                .profiles
                .profiles
                .iter_mut()
                .find(|profile| profile.id == loaded.state.active_profile_id)
            {
                let association = Some(WorkspaceAssociation {
                    root: workspace_root.clone(),
                });
                profile_changed |= profile.workspace != association;
                profile.workspace = association;
            }
            if profile_changed {
                loaded.state.profile_revision = loaded.state.profile_revision.saturating_add(1);
            }
            store.save(&loaded.state)?;
        }
        if bootstrap.workspace_root.is_none() {
            bootstrap.workspace_root = loaded.state.workspace_root.as_deref().map(PathBuf::from);
        }
        bootstrap.active_profile_id = Some(loaded.state.active_profile_id.clone());
        let mut session =
            Self::with_state(bootstrap, loaded.state, Some(store), Some(host_context));
        if let Some(message) = loaded.recovery_message {
            session.catalog_error = Some(message);
        }
        Ok(session)
    }

    fn with_state(
        bootstrap: ServiceBootstrap,
        durable_state: StudioPersistentState,
        state_store: Option<StudioStateStore>,
        host_context: Option<StudioHostContext>,
    ) -> Self {
        let active_profile_id = Some(durable_state.active_profile_id.clone());
        Self {
            bootstrap,
            revision: StudioRevision(0),
            active_profile_id,
            selected_content_id: None,
            catalog: Vec::new(),
            catalog_query: String::new(),
            catalog_error: None,
            workspace_content: WorkspaceContentSnapshot::default(),
            durable_state,
            state_store,
            host_context,
        }
    }

    pub fn snapshot(&self) -> StudioSnapshot {
        let mut available_actions = vec![AvailableAction {
            id: "catalog.refresh".to_owned(),
            enabled: true,
            reason: None,
        }];
        available_actions.extend([
            AvailableAction {
                id: "catalog.discover-content-projects".to_owned(),
                enabled: self.bootstrap.workspace_root.is_some(),
                reason: self
                    .bootstrap
                    .workspace_root
                    .is_none()
                    .then(|| "Open a workspace first.".to_owned()),
            },
            AvailableAction {
                id: "profile.select".to_owned(),
                enabled: true,
                reason: None,
            },
            AvailableAction {
                id: "catalog.search".to_owned(),
                enabled: true,
                reason: None,
            },
            AvailableAction {
                id: "catalog.inspect".to_owned(),
                enabled: !self.catalog.is_empty(),
                reason: self
                    .catalog
                    .is_empty()
                    .then(|| "Refresh the catalog first.".to_owned()),
            },
            AvailableAction {
                id: "workspace.open".to_owned(),
                enabled: true,
                reason: None,
            },
        ]);
        StudioSnapshot {
            revision: self.revision,
            host: HostSummary {
                app_root: self
                    .host_context
                    .as_ref()
                    .and_then(|context| context.app_root.as_ref())
                    .map(|root| root.display().to_string())
                    .or_else(|| {
                        self.bootstrap
                            .application_executable
                            .parent()
                            .and_then(|path| path.parent())
                            .and_then(|path| path.parent())
                            .map(|root| root.display().to_string())
                    }),
                workspace: self
                    .bootstrap
                    .workspace_root
                    .as_ref()
                    .map(|path| path.display().to_string()),
                mode: if self.bootstrap.fixture_mode {
                    "fixture".to_owned()
                } else {
                    "production".to_owned()
                },
                platform: self.host_context.as_ref().map_or_else(
                    || std::env::consts::OS.to_owned(),
                    |context| format!("{:?}", context.platform).to_lowercase(),
                ),
                steam_client_available: self
                    .host_context
                    .as_ref()
                    .is_some_and(|context| context.steam_client_available),
            },
            active_profile_id: self.active_profile_id.clone(),
            durable_state: DurableStateSnapshot {
                profile_revision: self.durable_state.profile_revision,
                profiles: self.durable_state.profiles.clone(),
                active_profile_id: self.durable_state.active_profile_id.clone(),
                workspace_root: self.durable_state.workspace_root.clone(),
                editor_drafts: self.durable_state.editor_drafts.clone(),
                operation_history: self.durable_state.operation_history.clone(),
                recovery_backup: self.durable_state.recovery_backup.clone(),
            },
            active_operation: None,
            catalog: self
                .catalog
                .iter()
                .filter(|entry| {
                    self.catalog_query.is_empty()
                        || entry.id.to_lowercase().contains(&self.catalog_query)
                        || entry.kind.to_lowercase().contains(&self.catalog_query)
                        || entry.version.as_deref().is_some_and(|version| {
                            version.to_lowercase().contains(&self.catalog_query)
                        })
                })
                .cloned()
                .collect(),
            workspace_content: self.workspace_content.clone(),
            selected_content_id: self.selected_content_id.clone(),
            available_actions,
        }
    }

    pub fn dispatch(&mut self, request: StudioRequest) -> StudioResponse {
        let mut response_kind = StudioResponseKind::Applied;
        let mut error = None;
        if request
            .expected_revision
            .is_some_and(|expected| expected != self.revision)
        {
            response_kind = StudioResponseKind::Rejected;
            error = Some(StudioError {
                code: StudioErrorCode::StaleRevision,
                message: format!(
                    "Studio state changed from revision {} to {}; refresh before retrying.",
                    request.expected_revision.map_or(0, |revision| revision.0),
                    self.revision.0
                ),
                retryable: true,
            });
        } else {
            match request.action {
                StudioAction::Catalog(CatalogAction::Refresh) => {
                    self.refresh_catalog();
                    self.revision.0 = self.revision.0.saturating_add(1);
                    if let Some(message) = self.catalog_error.clone() {
                        response_kind = StudioResponseKind::Rejected;
                        error = Some(StudioError {
                            code: StudioErrorCode::ProviderFailure,
                            message,
                            retryable: true,
                        });
                    }
                }
                StudioAction::Catalog(CatalogAction::DiscoverContentProjects)
                    if self.bootstrap.workspace_root.is_some() =>
                {
                    self.refresh_workspace_content();
                    self.revision.0 = self.revision.0.saturating_add(1);
                }
                StudioAction::Catalog(CatalogAction::DiscoverContentProjects) => {
                    response_kind = StudioResponseKind::Rejected;
                    error = Some(StudioError {
                        code: StudioErrorCode::ActionUnavailable,
                        message: "Open a workspace before discovering its content projects."
                            .to_owned(),
                        retryable: false,
                    });
                }
                StudioAction::Workspace(vapor_studio_protocol::WorkspaceAction::Open { root }) => {
                    let requested = PathBuf::from(root);
                    match std::fs::canonicalize(&requested) {
                        Ok(root) if root.is_dir() => {
                            let previous_state = self.durable_state.clone();
                            let previous_workspace = self.bootstrap.workspace_root.clone();
                            let root_changed =
                                self.bootstrap.workspace_root.as_deref() != Some(&root);
                            self.bootstrap.workspace_root = Some(root);
                            self.durable_state.workspace_root = self
                                .bootstrap
                                .workspace_root
                                .as_ref()
                                .map(|root| root.display().to_string());
                            let workspace = self
                                .bootstrap
                                .workspace_root
                                .clone()
                                .map(|root| WorkspaceAssociation { root });
                            let workspace_profile_changed = self
                                .durable_state
                                .profiles
                                .profiles
                                .iter()
                                .find(|profile| profile.id == self.durable_state.active_profile_id)
                                .is_some_and(|profile| profile.workspace != workspace);
                            if workspace_profile_changed {
                                self.durable_state.profile_revision =
                                    self.durable_state.profile_revision.saturating_add(1);
                            }
                            if workspace_profile_changed
                                && let Some(profile) = self.active_profile_mut()
                            {
                                profile.workspace = workspace;
                                profile.root_package = None;
                                profile.resolved_graph = None;
                                profile.installation = InstallationState::Unknown;
                            }
                            if let Some(store) = &self.state_store
                                && let Err(message) = store.save(&self.durable_state)
                            {
                                self.durable_state = previous_state;
                                self.bootstrap.workspace_root = previous_workspace;
                                response_kind = StudioResponseKind::Rejected;
                                error = Some(StudioError {
                                    code: StudioErrorCode::PersistenceFailure,
                                    message,
                                    retryable: true,
                                });
                                let snapshot = self.snapshot();
                                return StudioResponse {
                                    protocol_version: vapor_studio_protocol::PROTOCOL_VERSION,
                                    request_id: request.request_id,
                                    revision: snapshot.revision,
                                    kind: response_kind,
                                    snapshot,
                                    error,
                                };
                            }
                            self.selected_content_id = None;
                            if root_changed {
                                self.workspace_content = WorkspaceContentSnapshot::default();
                            }
                            self.refresh_catalog();
                            self.revision.0 = self.revision.0.saturating_add(1);
                            if let Some(message) = self.catalog_error.clone() {
                                response_kind = StudioResponseKind::Rejected;
                                error = Some(StudioError {
                                    code: StudioErrorCode::ProviderFailure,
                                    message,
                                    retryable: true,
                                });
                            }
                        }
                        Ok(root) => {
                            response_kind = StudioResponseKind::Rejected;
                            error = Some(StudioError {
                                code: StudioErrorCode::InvalidRequest,
                                message: format!("{} is not a workspace folder", root.display()),
                                retryable: true,
                            });
                        }
                        Err(failure) => {
                            response_kind = StudioResponseKind::Rejected;
                            error = Some(StudioError {
                                code: StudioErrorCode::InvalidRequest,
                                message: format!("{}: {failure}", requested.display()),
                                retryable: true,
                            });
                        }
                    }
                }
                StudioAction::Catalog(CatalogAction::Search { query }) => {
                    self.catalog_query = query.trim().to_lowercase();
                    self.revision.0 = self.revision.0.saturating_add(1);
                }
                StudioAction::Catalog(CatalogAction::Inspect {
                    content_id,
                    version,
                }) => {
                    if let Some(entry) = self
                        .catalog
                        .iter()
                        .find(|entry| entry.id == content_id && entry.version == version)
                    {
                        self.selected_content_id = Some(match entry.version.as_deref() {
                            Some(version) => format!("{}@{version}", entry.id),
                            None => entry.id.clone(),
                        });
                        self.revision.0 = self.revision.0.saturating_add(1);
                    } else {
                        response_kind = StudioResponseKind::Rejected;
                        error = Some(StudioError {
                            code: StudioErrorCode::ActionUnavailable,
                            message:
                                "That content identity is not in the current catalog snapshot."
                                    .to_owned(),
                            retryable: true,
                        });
                    }
                }
                StudioAction::Session(SessionAction::CheckpointLocalState {
                    editor_drafts,
                    operation_history,
                }) => {
                    let previous_state = self.durable_state.clone();
                    self.durable_state.editor_drafts = editor_drafts;
                    self.durable_state.operation_history = operation_history;
                    if let Some(store) = &self.state_store
                        && let Err(message) = store.save(&self.durable_state)
                    {
                        self.durable_state = previous_state;
                        response_kind = StudioResponseKind::Rejected;
                        error = Some(StudioError {
                            code: StudioErrorCode::PersistenceFailure,
                            message,
                            retryable: true,
                        });
                    } else {
                        self.revision.0 = self.revision.0.saturating_add(1);
                    }
                }
                StudioAction::Profile(action) => match self.apply_profile_action(action) {
                    Ok(()) => self.revision.0 = self.revision.0.saturating_add(1),
                    Err(profile_error) => {
                        response_kind = StudioResponseKind::Rejected;
                        error = Some(profile_error);
                    }
                },
                action => {
                    response_kind = StudioResponseKind::Rejected;
                    error = Some(StudioError {
                        code: StudioErrorCode::ActionUnavailable,
                        message: format!(
                            "Action '{}' is not connected to the service yet.",
                            action.id()
                        ),
                        retryable: false,
                    });
                }
            }
        }
        let snapshot = self.snapshot();
        StudioResponse {
            protocol_version: vapor_studio_protocol::PROTOCOL_VERSION,
            request_id: request.request_id,
            revision: snapshot.revision,
            kind: response_kind,
            snapshot,
            error,
        }
    }

    fn refresh_catalog(&mut self) {
        self.refresh_workspace_content();
        if self.bootstrap.fixture_mode {
            self.catalog_query.clear();
            self.catalog = vec![CatalogEntry {
                id: "fixture.engine".to_owned(),
                version: Some("1.0.0".to_owned()),
                kind: "engine".to_owned(),
                published_file_id: None,
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                locations: vec![CatalogLocation {
                    kind: CatalogLocationKind::Cache,
                    root: "fixture/cache/fixture.engine".to_owned(),
                    enabled: true,
                    fingerprint: Some("fixture:v1".to_owned()),
                    install_source: None,
                }],
            }];
            self.catalog_error = None;
            return;
        }
        match discover_content_catalog(
            &self.bootstrap.application_executable,
            self.bootstrap.workspace_root.as_deref(),
        ) {
            Ok(catalog) => {
                self.catalog_query.clear();
                self.catalog = catalog
                    .into_iter()
                    .take(MAX_CATALOG_ITEMS)
                    .map(catalog_entry)
                    .collect();
                self.catalog_error = None;
            }
            Err(error) => self.catalog_error = Some(error),
        }
    }

    fn refresh_workspace_content(&mut self) {
        let Some(root) = self.bootstrap.workspace_root.clone() else {
            self.workspace_content = WorkspaceContentSnapshot {
                source_projects_loaded: true,
                unregistered_projects_loaded: true,
                ..WorkspaceContentSnapshot::default()
            };
            return;
        };
        if self.bootstrap.fixture_mode {
            self.workspace_content = fixture_workspace_content(&root);
            return;
        }

        self.workspace_content.workspace_validation_error =
            vapor_studio_core::validate_workspace(&root).err();

        match discover_source_content(&self.bootstrap.application_executable, &root) {
            Ok(projects) => {
                self.workspace_content.source_projects = projects
                    .into_iter()
                    .take(MAX_WORKSPACE_PROJECTS)
                    .map(source_project)
                    .collect();
                self.workspace_content.source_projects_loaded = true;
                self.workspace_content.source_projects_error = None;
            }
            Err(error) => self.workspace_content.source_projects_error = Some(error),
        }

        match scan_unregistered_content_projects(&self.bootstrap.application_executable, &root) {
            Ok(scan) => {
                self.workspace_content.unregistered_projects = scan
                    .candidates
                    .into_iter()
                    .take(MAX_WORKSPACE_PROJECTS)
                    .map(|candidate| WorkspaceProjectCandidate {
                        path: candidate.path.display().to_string(),
                        id: candidate.id,
                        kind: candidate.kind.to_string(),
                    })
                    .collect();
                self.workspace_content.unregistered_diagnostics = scan
                    .diagnostics
                    .into_iter()
                    .take(MAX_WORKSPACE_DIAGNOSTICS)
                    .collect();
                self.workspace_content.unregistered_projects_loaded = true;
                self.workspace_content.unregistered_projects_error = None;
            }
            Err(error) => self.workspace_content.unregistered_projects_error = Some(error),
        }
    }

    fn active_profile_mut(&mut self) -> Option<&mut CompositionProfile> {
        let active = self.durable_state.active_profile_id.clone();
        self.durable_state
            .profiles
            .profiles
            .iter_mut()
            .find(|profile| profile.id == active)
    }

    fn apply_profile_action(&mut self, action: ProfileAction) -> Result<(), StudioError> {
        let previous_state = self.durable_state.clone();
        let previous_workspace = self.bootstrap.workspace_root.clone();
        let transition = match action {
            ProfileAction::Create { name } => add_profile(
                &mut self.durable_state.profiles.profiles,
                &name,
                self.bootstrap.workspace_root.clone(),
            )
            .map(|index| {
                self.durable_state.active_profile_id =
                    self.durable_state.profiles.profiles[index].id.clone();
            }),
            ProfileAction::Duplicate { profile_id, name } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .and_then(|index| {
                    duplicate_profile(&mut self.durable_state.profiles.profiles, index)
                })
                .and_then(|index| {
                    if name.trim().is_empty() {
                        Ok(index)
                    } else {
                        rename_profile(&mut self.durable_state.profiles.profiles, index, &name)?;
                        Ok(index)
                    }
                })
                .map(|index| {
                    self.durable_state.active_profile_id =
                        self.durable_state.profiles.profiles[index].id.clone();
                }),
            ProfileAction::Rename { profile_id, name } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .and_then(|index| {
                    rename_profile(&mut self.durable_state.profiles.profiles, index, &name)
                }),
            ProfileAction::Remove { profile_id } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .and_then(|index| {
                    remove_profile(&mut self.durable_state.profiles.profiles, index).map(|_| ())
                })
                .map(|()| {
                    if self.durable_state.active_profile_id == profile_id {
                        self.durable_state.active_profile_id =
                            self.durable_state.profiles.profiles[0].id.clone();
                    }
                }),
            ProfileAction::Select { profile_id } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .map(|index| {
                    self.durable_state.active_profile_id =
                        self.durable_state.profiles.profiles[index].id.clone();
                }),
            ProfileAction::SetRoot {
                profile_id,
                content_id,
                content_version,
                workspace_root,
            } => self
                .profile_index(&profile_id)
                .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                .map(|index| {
                    self.durable_state.profiles.profiles[index].root_package =
                        content_id.map(|id| ContentIdentity {
                            id,
                            version: content_version,
                        });
                    self.durable_state.profiles.profiles[index].resolved_graph = None;
                    self.durable_state.profiles.profiles[index].installation =
                        InstallationState::Unknown;
                    self.durable_state.profiles.profiles[index].workspace = workspace_root
                        .map(PathBuf::from)
                        .map(|root| WorkspaceAssociation { root });
                }),
            ProfileAction::SetLaunchArguments {
                profile_id,
                arguments,
            } => {
                if arguments.len() > 64 || arguments.iter().any(|argument| argument.len() > 4096) {
                    Err(
                        "A profile can have at most 64 launch arguments, each 4096 bytes or fewer."
                            .to_owned(),
                    )
                } else {
                    self.profile_index(&profile_id)
                        .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                        .map(|index| {
                            self.durable_state.profiles.profiles[index].launch.arguments =
                                arguments;
                        })
                }
            }
            ProfileAction::SetCompositionState {
                profile_id,
                root_package,
                resolved_graph,
                installation,
            } => {
                if resolved_graph.as_ref().is_some_and(|graph| {
                    root_package.as_ref().is_none_or(|root| {
                        graph.root.id != root.id
                            || root
                                .version
                                .as_ref()
                                .is_some_and(|version| graph.root.version.as_ref() != Some(version))
                    })
                }) {
                    Err(
                        "The resolved graph root must match the profile's selected root."
                            .to_owned(),
                    )
                } else {
                    self.profile_index(&profile_id)
                        .ok_or_else(|| "The selected profile no longer exists.".to_owned())
                        .map(|index| {
                            let profile = &mut self.durable_state.profiles.profiles[index];
                            profile.root_package = root_package;
                            profile.resolved_graph = resolved_graph.map(|graph| *graph);
                            profile.installation = *installation;
                        })
                }
            }
            ProfileAction::InvalidateContent {
                content_id,
                content_version,
            } => {
                let matches_content = |identity: &ContentIdentity| {
                    identity.id == content_id
                        && content_version
                            .as_ref()
                            .is_none_or(|version| identity.version.as_ref() == Some(version))
                };
                for profile in &mut self.durable_state.profiles.profiles {
                    let affected = profile.root_package.as_ref().is_some_and(&matches_content)
                        || profile.resolved_graph.as_ref().is_some_and(|graph| {
                            graph
                                .nodes
                                .iter()
                                .any(|node| matches_content(&node.content))
                        });
                    if affected {
                        profile.resolved_graph = None;
                        profile.installation = InstallationState::Unknown;
                    }
                }
                Ok(())
            }
        };
        if let Err(message) = transition {
            self.durable_state = previous_state;
            self.active_profile_id = Some(self.durable_state.active_profile_id.clone());
            self.bootstrap.workspace_root = previous_workspace;
            return Err(StudioError {
                code: StudioErrorCode::InvalidRequest,
                message,
                retryable: true,
            });
        }
        self.durable_state.profile_revision = previous_state.profile_revision.saturating_add(1);
        self.active_profile_id = Some(self.durable_state.active_profile_id.clone());
        self.bootstrap.workspace_root = self.active_profile_mut().and_then(|profile| {
            profile
                .workspace
                .as_ref()
                .map(|workspace| workspace.root.clone())
        });
        self.durable_state.workspace_root = self
            .bootstrap
            .workspace_root
            .as_ref()
            .map(|root| root.display().to_string());
        let workspace_changed = previous_workspace != self.bootstrap.workspace_root;
        if workspace_changed {
            self.selected_content_id = None;
            self.workspace_content = WorkspaceContentSnapshot::default();
            self.refresh_catalog();
        }
        if let Some(store) = &self.state_store
            && let Err(message) = store.save(&self.durable_state)
        {
            self.durable_state = previous_state;
            self.active_profile_id = Some(self.durable_state.active_profile_id.clone());
            self.bootstrap.workspace_root = previous_workspace;
            return Err(StudioError {
                code: StudioErrorCode::PersistenceFailure,
                message,
                retryable: true,
            });
        }
        Ok(())
    }

    fn profile_index(&self, profile_id: &str) -> Option<usize> {
        self.durable_state
            .profiles
            .profiles
            .iter()
            .position(|profile| profile.id == profile_id)
    }
}

fn source_project(project: SourceContent) -> WorkspaceSourceProject {
    WorkspaceSourceProject {
        id: project.id,
        version: project.version,
        kind: project.kind,
        root: project.root.display().to_string(),
        manifest: project.manifest.display().to_string(),
        binaries: project.binaries,
        libraries: project.libraries,
        dependencies: project.dependencies,
        workshop: WorkspaceWorkshopPolicy {
            app_id: project.workshop.app_id,
            published_file_id: project.workshop.published_file_id,
            visibility: project.workshop.visibility,
            title: project.workshop.title,
            description: project.workshop.description,
            tags: project.workshop.tags,
            change_note: project.workshop.change_note,
        },
    }
}

fn fixture_workspace_content(root: &Path) -> WorkspaceContentSnapshot {
    WorkspaceContentSnapshot {
        source_projects: vec![WorkspaceSourceProject {
            id: "fixture.game".to_owned(),
            version: Some("1.0.0".to_owned()),
            kind: "game".to_owned(),
            root: root.join("game").display().to_string(),
            manifest: root.join("game/Vapor.toml").display().to_string(),
            binaries: vec!["fixture-game".to_owned()],
            libraries: Vec::new(),
            dependencies: vec!["required · fixture.engine".to_owned()],
            workshop: WorkspaceWorkshopPolicy::default(),
        }],
        source_projects_loaded: true,
        source_projects_error: None,
        unregistered_projects: vec![WorkspaceProjectCandidate {
            path: root.join("unregistered-addon").display().to_string(),
            id: "fixture.addon".to_owned(),
            kind: "game-mod".to_owned(),
        }],
        unregistered_diagnostics: vec!["fixture diagnostic".to_owned()],
        unregistered_projects_loaded: true,
        unregistered_projects_error: None,
        workspace_validation_error: None,
    }
}

fn catalog_entry(entry: CatalogContent) -> CatalogEntry {
    CatalogEntry {
        id: entry.id,
        version: entry.version,
        kind: entry.kind,
        published_file_id: entry.published_file_id,
        dependencies: entry.dependencies,
        conflicts: entry
            .conflicts
            .into_iter()
            .map(|conflict| CatalogConflict {
                field: conflict.field,
                first_value: conflict.first_value,
                first_location: conflict.first_location.display().to_string(),
                conflicting_value: conflict.conflicting_value,
                conflicting_location: conflict.conflicting_location.display().to_string(),
            })
            .collect(),
        locations: entry
            .locations
            .into_iter()
            .map(|location| CatalogLocation {
                kind: match location.kind {
                    vapor_studio_core::CatalogLocationKind::Workspace => {
                        CatalogLocationKind::Workspace
                    }
                    vapor_studio_core::CatalogLocationKind::Cache => CatalogLocationKind::Cache,
                    vapor_studio_core::CatalogLocationKind::Installed => {
                        CatalogLocationKind::Installed
                    }
                },
                root: location.root.display().to_string(),
                enabled: location.enabled,
                fingerprint: location.fingerprint,
                install_source: location.install_source,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vapor_studio_protocol::{RequestId, StudioAction, StudioRequest};

    fn session() -> StudioSession {
        StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workspace_root: None,
            active_profile_id: Some("default".to_owned()),
            fixture_mode: true,
        })
    }

    fn request(revision: Option<u64>, action: StudioAction) -> StudioRequest {
        StudioRequest {
            protocol_version: vapor_studio_protocol::PROTOCOL_VERSION,
            request_id: RequestId("test-request".to_owned()),
            expected_revision: revision.map(StudioRevision),
            action,
        }
    }

    #[test]
    fn fixture_catalog_refresh_is_deterministic_and_advances_revision() {
        let mut session = session();
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::Refresh),
        ));
        assert_eq!(response.kind, StudioResponseKind::Applied);
        assert_eq!(response.revision, StudioRevision(1));
        assert_eq!(response.snapshot.catalog[0].id, "fixture.engine");
        assert_eq!(response.snapshot.host.mode, "fixture");
        assert!(response.snapshot.workspace_content.source_projects_loaded);
        assert!(
            response
                .snapshot
                .workspace_content
                .unregistered_projects_loaded
        );
    }

    #[test]
    fn workspace_project_discovery_is_a_service_action() {
        let mut session = StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workspace_root: Some(PathBuf::from("/fixture/workspace")),
            active_profile_id: None,
            fixture_mode: true,
        });
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::DiscoverContentProjects),
        ));

        assert_eq!(response.kind, StudioResponseKind::Applied);
        assert_eq!(response.revision, StudioRevision(1));
        assert_eq!(
            response.snapshot.workspace_content.source_projects[0].id,
            "fixture.game"
        );
        assert_eq!(
            response.snapshot.workspace_content.unregistered_projects[0].id,
            "fixture.addon"
        );
    }

    #[test]
    fn workspace_discovery_is_unavailable_until_a_workspace_is_open() {
        let mut session = session();
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::DiscoverContentProjects),
        ));

        assert_eq!(response.kind, StudioResponseKind::Rejected);
        assert_eq!(response.revision, StudioRevision(0));
        assert_eq!(
            response.error.unwrap().code,
            StudioErrorCode::ActionUnavailable
        );
        let action = response
            .snapshot
            .available_actions
            .into_iter()
            .find(|action| action.id == "catalog.discover-content-projects")
            .expect("availability projection contains workspace discovery");
        assert!(!action.enabled);
        assert_eq!(action.reason.as_deref(), Some("Open a workspace first."));
    }

    #[test]
    fn workspace_open_returns_source_and_unregistered_projects_in_its_snapshot() {
        let mut session = StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workspace_root: None,
            active_profile_id: None,
            fixture_mode: true,
        });
        let root = std::env::current_dir().expect("current directory");
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Workspace(vapor_studio_protocol::WorkspaceAction::Open {
                root: root.display().to_string(),
            }),
        ));

        assert_eq!(response.kind, StudioResponseKind::Applied);
        let content = response.snapshot.workspace_content;
        assert!(content.source_projects_loaded);
        assert_eq!(content.source_projects[0].id, "fixture.game");
        assert!(content.unregistered_projects_loaded);
        assert_eq!(content.unregistered_projects[0].id, "fixture.addon");
        assert_eq!(content.unregistered_diagnostics, ["fixture diagnostic"]);
    }

    #[test]
    fn switching_workspace_clears_the_old_composition_in_service_state() {
        let mut session = StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workspace_root: None,
            active_profile_id: None,
            fixture_mode: true,
        });
        let assigned = session.dispatch(request(
            Some(0),
            StudioAction::Profile(ProfileAction::SetCompositionState {
                profile_id: "default".to_owned(),
                root_package: Some(ContentIdentity {
                    id: "old.game".to_owned(),
                    version: Some("1.0".to_owned()),
                }),
                resolved_graph: None,
                installation: Box::new(InstallationState::NotInstalled),
            }),
        ));
        assert_eq!(assigned.kind, StudioResponseKind::Applied);

        let root = std::env::current_dir().expect("current directory");
        let opened = session.dispatch(request(
            Some(1),
            StudioAction::Workspace(vapor_studio_protocol::WorkspaceAction::Open {
                root: root.display().to_string(),
            }),
        ));
        assert_eq!(opened.kind, StudioResponseKind::Applied);
        let profile = &opened.snapshot.durable_state.profiles.profiles[0];
        assert_eq!(
            profile.workspace.as_ref().map(|value| &value.root),
            Some(&root)
        );
        assert_eq!(profile.root_package, None);
        assert_eq!(profile.resolved_graph, None);
        assert_eq!(profile.installation, InstallationState::Unknown);
    }

    #[test]
    fn catalog_search_filters_snapshot_without_destroying_catalog() {
        let mut session = session();
        let _ = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::Refresh),
        ));
        let filtered = session.dispatch(request(
            Some(1),
            StudioAction::Catalog(CatalogAction::Search {
                query: "missing".into(),
            }),
        ));
        assert!(filtered.snapshot.catalog.is_empty());
        let restored = session.dispatch(request(
            Some(2),
            StudioAction::Catalog(CatalogAction::Search {
                query: String::new(),
            }),
        ));
        assert_eq!(restored.snapshot.catalog.len(), 1);
    }

    #[test]
    fn stale_action_is_rejected_without_mutating_state() {
        let mut session = session();
        let _ = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::Refresh),
        ));
        let response = session.dispatch(request(
            Some(0),
            StudioAction::Catalog(CatalogAction::Inspect {
                content_id: "fixture.engine".to_owned(),
                version: Some("1.0.0".to_owned()),
            }),
        ));
        assert_eq!(response.kind, StudioResponseKind::Rejected);
        assert_eq!(response.error.unwrap().code, StudioErrorCode::StaleRevision);
        assert_eq!(response.revision, StudioRevision(1));
        assert_eq!(response.snapshot.selected_content_id, None);
    }

    #[test]
    fn profile_selection_requires_an_existing_profile() {
        let mut session = StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workspace_root: None,
            active_profile_id: None,
            fixture_mode: false,
        });
        let created = session.dispatch(request(
            Some(0),
            StudioAction::Profile(ProfileAction::Create {
                name: "Second".to_owned(),
            }),
        ));
        assert_eq!(created.kind, StudioResponseKind::Applied);
        assert_eq!(created.revision, StudioRevision(1));
        assert_eq!(
            created.snapshot.active_profile_id.as_deref(),
            Some("profile-1")
        );
        assert_eq!(created.snapshot.durable_state.profiles.profiles.len(), 2);
        assert_eq!(created.snapshot.durable_state.profile_revision, 1);
        let rejected = session.dispatch(request(
            Some(1),
            StudioAction::Profile(ProfileAction::Select {
                profile_id: "unknown".to_owned(),
            }),
        ));
        assert_eq!(rejected.kind, StudioResponseKind::Rejected);
        assert_eq!(rejected.revision, StudioRevision(1));
    }

    #[test]
    fn profile_launch_arguments_are_validated_and_profile_revision_is_projected() {
        let mut session = session();
        let update = session.dispatch(request(
            Some(0),
            StudioAction::Profile(ProfileAction::SetLaunchArguments {
                profile_id: "default".to_owned(),
                arguments: vec!["--windowed".to_owned()],
            }),
        ));
        assert_eq!(update.kind, StudioResponseKind::Applied);
        assert_eq!(update.snapshot.durable_state.profile_revision, 1);
        assert_eq!(
            update.snapshot.durable_state.profiles.profiles[0]
                .launch
                .arguments,
            ["--windowed"]
        );

        let rejected = session.dispatch(request(
            Some(1),
            StudioAction::Profile(ProfileAction::SetLaunchArguments {
                profile_id: "default".to_owned(),
                arguments: vec!["x".repeat(4097)],
            }),
        ));
        assert_eq!(rejected.kind, StudioResponseKind::Rejected);
        assert_eq!(rejected.snapshot.durable_state.profile_revision, 1);
        assert_eq!(
            rejected.snapshot.durable_state.profiles.profiles[0]
                .launch
                .arguments,
            ["--windowed"]
        );
    }

    #[test]
    fn content_invalidation_clears_stale_graph_and_installation_without_removing_root() {
        let mut session = session();
        let root = ContentIdentity {
            id: "fixture.game".to_owned(),
            version: Some("1.0.0".to_owned()),
        };
        let assigned = session.dispatch(request(
            Some(0),
            StudioAction::Profile(ProfileAction::SetCompositionState {
                profile_id: "default".to_owned(),
                root_package: Some(root.clone()),
                resolved_graph: None,
                installation: Box::new(InstallationState::NotInstalled),
            }),
        ));
        assert_eq!(assigned.kind, StudioResponseKind::Applied);

        let invalidated = session.dispatch(request(
            Some(1),
            StudioAction::Profile(ProfileAction::InvalidateContent {
                content_id: root.id.clone(),
                content_version: root.version.clone(),
            }),
        ));
        assert_eq!(invalidated.kind, StudioResponseKind::Applied);
        let profile = &invalidated.snapshot.durable_state.profiles.profiles[0];
        assert_eq!(profile.root_package, Some(root));
        assert_eq!(profile.resolved_graph, None);
        assert_eq!(profile.installation, InstallationState::Unknown);
    }
}
