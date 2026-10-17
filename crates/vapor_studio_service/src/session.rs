use std::path::PathBuf;
use vapor_studio_core::{CatalogContent, discover_content_catalog};
use vapor_studio_protocol::{
    AvailableAction, CatalogAction, CatalogConflict, CatalogEntry, CatalogLocation,
    CatalogLocationKind, HostSummary, StudioAction, StudioError, StudioErrorCode, StudioRequest,
    StudioResponse, StudioResponseKind, StudioRevision, StudioSnapshot,
};

const MAX_CATALOG_ITEMS: usize = 10_000;

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
}

impl StudioSession {
    pub fn new(bootstrap: ServiceBootstrap) -> Self {
        let active_profile_id = bootstrap.active_profile_id.clone();
        Self {
            bootstrap,
            revision: StudioRevision(0),
            active_profile_id,
            selected_content_id: None,
            catalog: Vec::new(),
            catalog_query: String::new(),
            catalog_error: None,
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
                    .bootstrap
                    .application_executable
                    .parent()
                    .and_then(|path| path.parent())
                    .and_then(|path| path.parent())
                    .map(|root| root.display().to_string()),
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
            },
            active_profile_id: self.active_profile_id.clone(),
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
                StudioAction::Workspace(vapor_studio_protocol::WorkspaceAction::Open { root }) => {
                    let requested = PathBuf::from(root);
                    match std::fs::canonicalize(&requested) {
                        Ok(root) if root.is_dir() => {
                            self.bootstrap.workspace_root = Some(root);
                            self.selected_content_id = None;
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
                StudioAction::Profile(vapor_studio_protocol::ProfileAction::Select {
                    profile_id,
                }) if !profile_id.trim().is_empty() => {
                    self.active_profile_id = Some(profile_id);
                    self.revision.0 = self.revision.0.saturating_add(1);
                }
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
    fn profile_selection_rejects_empty_ids_and_changes_active_profile() {
        let mut session = StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workspace_root: None,
            active_profile_id: None,
            fixture_mode: false,
        });
        let accepted = session.dispatch(request(
            Some(0),
            StudioAction::Profile(vapor_studio_protocol::ProfileAction::Select {
                profile_id: "profile-2".to_owned(),
            }),
        ));
        assert_eq!(accepted.kind, StudioResponseKind::Applied);
        assert_eq!(accepted.revision, StudioRevision(1));
        assert_eq!(
            accepted.snapshot.active_profile_id.as_deref(),
            Some("profile-2")
        );
        let rejected = session.dispatch(request(
            Some(1),
            StudioAction::Profile(vapor_studio_protocol::ProfileAction::Select {
                profile_id: "  ".to_owned(),
            }),
        ));
        assert_eq!(rejected.kind, StudioResponseKind::Rejected);
        assert_eq!(rejected.revision, StudioRevision(1));
    }
}
