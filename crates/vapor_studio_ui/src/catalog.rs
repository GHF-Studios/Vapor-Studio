use super::StudioApp;
use crate::navigation::CatalogSourceFilter;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use vapor_core::ContentKind;
use vapor_studio_core::ContentIdentity;
use vapor_studio_protocol::{
    CatalogEntry as ServiceCatalogEntry, CatalogLocationKind as ServiceLocationKind, ProtocolFrame,
    StudioAction, StudioEvent, StudioResponseKind,
};

pub(super) struct CatalogState {
    pub(super) discover_tab: DiscoverTab,
    pub(super) search_query: String,
    pub(super) sources: Vec<vapor_studio_core::SourceContent>,
    pub(super) source_discovery_loaded: bool,
    pub(super) source_discovery_error: Option<String>,
    pub(super) items: Vec<vapor_studio_core::CatalogContent>,
    pub(super) catalog_discovery_loaded: bool,
    pub(super) catalog_discovery_error: Option<String>,
    pub(super) source_filter: CatalogSourceFilter,
    pub(super) kind_filter: Option<String>,
    pub(super) selected: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum DiscoverTab {
    #[default]
    Local,
    Workshop,
}

impl Default for CatalogState {
    fn default() -> Self {
        Self {
            discover_tab: DiscoverTab::Local,
            search_query: String::new(),
            sources: Vec::new(),
            source_discovery_loaded: false,
            source_discovery_error: None,
            items: Vec::new(),
            catalog_discovery_loaded: false,
            catalog_discovery_error: None,
            source_filter: CatalogSourceFilter::All,
            kind_filter: None,
            selected: None,
        }
    }
}

impl CatalogState {
    pub(super) fn source_snapshot_current(&self) -> bool {
        self.source_discovery_loaded && self.source_discovery_error.is_none()
    }

    pub(super) fn local_catalog_current(&self) -> bool {
        self.catalog_discovery_loaded && self.catalog_discovery_error.is_none()
    }

    pub(super) fn clear_workspace_content(&mut self) {
        self.sources.clear();
        self.source_discovery_loaded = false;
        self.source_discovery_error = None;
        self.items.clear();
        self.catalog_discovery_loaded = false;
        self.catalog_discovery_error = None;
        self.selected = None;
    }
}

impl StudioApp {
    pub(super) fn select_catalog_identity(&mut self, selector: String) {
        let (content_id, version) = selector
            .split_once('@')
            .map(|(id, version)| (id.to_owned(), Some(version.to_owned())))
            .unwrap_or_else(|| (selector.clone(), None));
        self.catalog.selected = Some(selector);
        self.submit_service_action(StudioAction::Catalog(
            vapor_studio_protocol::CatalogAction::Inspect {
                content_id,
                version,
            },
        ));
    }

    pub(super) fn apply_service_action(&mut self, action: StudioAction) -> Result<(), String> {
        let Some(service) = self.service.as_mut() else {
            return Err(self
                .service_error
                .clone()
                .unwrap_or_else(|| "Studio service is not connected.".to_owned()));
        };
        let request_id = service
            .send(action, None)
            .map_err(|error| format!("Could not send the action to Studio: {error}"))?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Studio did not finish saving the change in time.".to_owned());
            }
            let frame = self
                .service
                .as_mut()
                .ok_or_else(|| "Studio service disconnected.".to_owned())?
                .recv_timeout(remaining)
                .map_err(|error| format!("Studio could not confirm the change: {error}"))?;
            match frame {
                ProtocolFrame::Response(response) if response.request_id == request_id => {
                    self.apply_service_catalog_snapshot(&response.snapshot);
                    return if response.kind == StudioResponseKind::Applied {
                        Ok(())
                    } else {
                        Err(response.error.map_or_else(
                            || "Studio rejected the change.".to_owned(),
                            |error| error.message,
                        ))
                    };
                }
                ProtocolFrame::Response(response) => {
                    self.apply_service_catalog_snapshot(&response.snapshot);
                }
                ProtocolFrame::Event(StudioEvent::Snapshot(snapshot)) => {
                    self.apply_service_catalog_snapshot(&snapshot);
                }
                ProtocolFrame::Event(StudioEvent::OperationChanged(event)) => {
                    self.apply_service_operation_event(&event);
                }
                ProtocolFrame::Event(StudioEvent::ProtocolError(error)) => {
                    return Err(error.message);
                }
                _ => {}
            }
        }
    }

    pub(super) fn persist_session_state(&mut self) -> Result<(), String> {
        let editor_drafts = self
            .workspace
            .editor
            .save_editor_drafts(&self.workspace.root);
        let operation_history = serde_json::to_string(&self.operation.operation_history).ok();
        self.apply_service_action(StudioAction::Session(
            vapor_studio_protocol::SessionAction::CheckpointLocalState {
                editor_drafts,
                operation_history,
            },
        ))
    }

    pub(super) fn submit_service_action(&mut self, action: StudioAction) {
        let Some(service) = self.service.as_mut() else {
            if self.service_error.is_none() {
                self.service_error = Some("Studio service is not connected.".to_owned());
            }
            self.catalog.catalog_discovery_loaded = false;
            self.catalog.catalog_discovery_error = self.service_error.clone();
            return;
        };
        if let Err(error) = service.send(action, None) {
            let message = format!("Could not send the action to the Studio service: {error}");
            self.service_error = Some(message.clone());
            self.catalog.catalog_discovery_error = Some(message);
        }
    }

    pub(super) fn poll_service(&mut self) {
        let mut frames = Vec::new();
        let mut disconnected = None;
        if let Some(service) = self.service.as_mut() {
            loop {
                match service.try_recv() {
                    Ok(Some(frame)) => frames.push(frame),
                    Ok(None) => break,
                    Err(error) => {
                        disconnected = Some(error);
                        break;
                    }
                }
            }
        } else {
            return;
        }
        for frame in frames {
            match frame {
                ProtocolFrame::Response(response) => {
                    self.apply_service_catalog_snapshot(&response.snapshot);
                    let composition_request =
                        self.operation.service_request_id.as_ref() == Some(&response.request_id);
                    if composition_request {
                        self.operation.service_request_id = None;
                        if response.kind != StudioResponseKind::OperationAccepted {
                            self.operation.running = false;
                            self.operation.service_owned = false;
                            self.operation.active_operation = None;
                        }
                    }
                    match response.error {
                        Some(error) => {
                            if composition_request {
                                self.status = error.message.clone();
                            }
                            self.catalog.catalog_discovery_error = Some(error.message.clone());
                            self.service_error = Some(error.message);
                        }
                        None if response.kind == StudioResponseKind::Applied => {
                            self.catalog.catalog_discovery_error = None;
                            self.service_error = None;
                        }
                        None => {}
                    }
                }
                ProtocolFrame::Event(StudioEvent::Snapshot(snapshot)) => {
                    self.apply_service_catalog_snapshot(&snapshot);
                }
                ProtocolFrame::Event(StudioEvent::OperationChanged(event)) => {
                    self.apply_service_operation_event(&event);
                }
                ProtocolFrame::Event(StudioEvent::ProtocolError(error)) => {
                    self.catalog.catalog_discovery_error = Some(error.message.clone());
                    if error.code == vapor_studio_protocol::StudioErrorCode::ProviderFailure {
                        self.catalog.catalog_discovery_loaded = false;
                    }
                    self.service_error = Some(error.message);
                }
                _ => {}
            }
        }
        if let Some(error) = disconnected {
            let message = format!("Studio service disconnected: {error}");
            if self.operation.service_owned {
                self.operation.running = false;
                self.operation.service_owned = false;
                self.operation.service_request_id = None;
                self.operation.active_operation = None;
            }
            self.catalog.catalog_discovery_error = Some(message.clone());
            self.service_error = Some(message);
            self.service = None;
        }
    }

    pub(super) fn apply_service_catalog_snapshot(
        &mut self,
        snapshot: &vapor_studio_protocol::StudioSnapshot,
    ) {
        let durable = &snapshot.durable_state;
        if !durable.profiles.profiles.is_empty()
            && self.profile.service_revision != Some(durable.profile_revision)
        {
            self.profile.items = durable.profiles.profiles.clone();
            self.profile.active_index = self
                .profile
                .items
                .iter()
                .position(|profile| profile.id == durable.active_profile_id)
                .unwrap_or(0);
            self.profile.name_draft = self.profile.items[self.profile.active_index].name.clone();
            self.profile.recovery_backup = durable.recovery_backup.clone();
            self.profile.service_revision = Some(durable.profile_revision);
        }
        let service_workspace = durable.workspace_root.clone().unwrap_or_default();
        if self.workspace.root != service_workspace && !self.workspace.editor.has_dirty_documents()
        {
            self.workspace.root = service_workspace.clone();
            self.workspace.root_draft = if service_workspace.is_empty() {
                self.workspace.fallback_root.display().to_string()
            } else {
                service_workspace
            };
        }
        self.catalog.items = snapshot.catalog.iter().map(service_catalog_entry).collect();
        self.catalog.catalog_discovery_loaded = true;
        self.catalog.sources = snapshot
            .workspace_content
            .source_projects
            .iter()
            .map(service_source_project)
            .collect();
        self.catalog.source_discovery_loaded = snapshot.workspace_content.source_projects_loaded;
        self.catalog.source_discovery_error =
            snapshot.workspace_content.source_projects_error.clone();
        self.workspace.workspace_validation_error = snapshot
            .workspace_content
            .workspace_validation_error
            .clone();

        let mut candidates = Vec::new();
        let mut diagnostics = snapshot.workspace_content.unregistered_diagnostics.clone();
        for candidate in &snapshot.workspace_content.unregistered_projects {
            if let Some(candidate) = service_workspace_candidate(candidate) {
                candidates.push(candidate);
            } else {
                diagnostics.push(format!(
                    "Studio service returned unsupported content kind '{}' for {}.",
                    candidate.kind, candidate.id
                ));
            }
        }
        self.workspace.unregistered_projects = vapor_studio_core::UnregisteredProjectScan {
            candidates,
            diagnostics,
        };
        self.workspace.unregistered_scan_completed =
            snapshot.workspace_content.unregistered_projects_loaded;
        self.workspace.unregistered_scan_error = snapshot
            .workspace_content
            .unregistered_projects_error
            .clone();
        self.apply_service_operation_snapshot(snapshot.active_operation.as_ref());
        let selected_profile_id = self
            .profile
            .items
            .get(self.profile.active_index)
            .map(|profile| profile.id.as_str());
        if let Some(preview) = snapshot
            .composition_preview
            .as_ref()
            .filter(|preview| Some(preview.profile_id.as_str()) == selected_profile_id)
        {
            let preview = protocol_composition_preview(preview);
            if self.profile_composition.preview() != Some(&preview) {
                self.apply_composition_preview(preview);
            }
        } else if snapshot.composition_preview.is_none()
            && self.profile_composition.preview().is_some()
        {
            self.profile_composition.invalidate_preview();
        }
        if let Some(selected_id) = snapshot.selected_content_id.as_deref() {
            self.catalog.selected = self
                .catalog
                .items
                .iter()
                .find(|entry| catalog_content_selector_matches(entry, selected_id))
                .map(catalog_content_selector);
        }
        if self.catalog.selected.as_ref().is_some_and(|identity| {
            !self
                .catalog
                .items
                .iter()
                .any(|entry| catalog_content_selector_matches(entry, identity))
        }) {
            self.catalog.selected = None;
        }
        if self.catalog.local_catalog_current() && self.catalog.source_snapshot_current() {
            match self.profile_composition.take_repreview_request() {
                Some(super::composition_lifecycle::RepreviewAfterCatalogRefresh::ActiveProfile) => {
                    self.preview_selected_composition();
                }
                Some(super::composition_lifecycle::RepreviewAfterCatalogRefresh::Source {
                    selector,
                    workspace_root,
                }) => self.preview_composition(
                    selector,
                    Some(workspace_root),
                    "Re-resolving the source composition against the refreshed cache…",
                ),
                None => {}
            }
        }
    }

    pub(super) fn refresh_content_catalog(&mut self) {
        self.profile_composition.invalidate_preview();
        self.submit_service_action(StudioAction::Catalog(
            vapor_studio_protocol::CatalogAction::Refresh,
        ));
    }
}

fn protocol_composition_preview(
    preview: &vapor_studio_protocol::CompositionPreviewSnapshot,
) -> vapor_studio_core::CompositionPreview {
    vapor_studio_core::CompositionPreview {
        selector: preview.selector.clone(),
        nodes: preview
            .nodes
            .iter()
            .map(|node| vapor_studio_core::CompositionPreviewNode {
                id: node.id.clone(),
                version: node.version.clone(),
                kind: node.kind.clone(),
                published_file_id: node.published_file_id.clone(),
                source: match node.source {
                    vapor_studio_protocol::CompositionPreviewSource::Workspace => {
                        vapor_studio_core::CompositionPreviewSource::Workspace
                    }
                    vapor_studio_protocol::CompositionPreviewSource::Cache => {
                        vapor_studio_core::CompositionPreviewSource::Cache
                    }
                    vapor_studio_protocol::CompositionPreviewSource::Installed => {
                        vapor_studio_core::CompositionPreviewSource::Installed
                    }
                    vapor_studio_protocol::CompositionPreviewSource::DownloadRequired => {
                        vapor_studio_core::CompositionPreviewSource::DownloadRequired
                    }
                },
                already_installed: node.already_installed,
                dependencies: node
                    .dependencies
                    .iter()
                    .map(
                        |dependency| vapor_studio_core::CompositionPreviewDependency {
                            id: dependency.id.clone(),
                            relationship: dependency.relationship.clone(),
                            version: dependency.version.clone(),
                            optional: dependency.optional,
                        },
                    )
                    .collect(),
            })
            .collect(),
        blockers: preview.blockers.clone(),
        warnings: preview.warnings.clone(),
    }
}

pub(super) fn content_selector(identity: &ContentIdentity) -> String {
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

pub(crate) fn source_content_selector_matches(
    content: &vapor_studio_core::SourceContent,
    selector: &str,
) -> bool {
    content_selector_matches(&content.id, content.version.as_deref(), selector)
}

pub(crate) fn catalog_content_selector(content: &vapor_studio_core::CatalogContent) -> String {
    content_selector(&content.identity())
}

pub(crate) fn catalog_content_selector_matches(
    content: &vapor_studio_core::CatalogContent,
    selector: &str,
) -> bool {
    content_selector_matches(&content.id, content.version.as_deref(), selector)
}

fn content_selector_matches(id: &str, version: Option<&str>, selector: &str) -> bool {
    match version {
        Some(version) => {
            selector
                .strip_prefix(id)
                .and_then(|remainder| remainder.strip_prefix('@'))
                == Some(version)
        }
        None => selector == id,
    }
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
        "library" => ContentKind::Library,
        _ => return None,
    })
}

pub(crate) fn content_kind_title(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::Packagepack => "Playable game setup",
        ContentKind::Enginepack => "Engine setup",
        ContentKind::Gamepack => "Game setup collection",
        ContentKind::Modpack => "Add-on collection",
        ContentKind::Engine => "Game engine",
        ContentKind::Game => "Game content",
        ContentKind::EngineMod => "Engine add-on",
        ContentKind::GameMod => "Game add-on",
        ContentKind::ExtensionMod => "General add-on",
        ContentKind::Library => "Content project",
    }
}

pub(crate) fn display_content_kind(kind: &str) -> String {
    source_content_kind(kind)
        .map(content_kind_title)
        .unwrap_or(kind)
        .to_owned()
}

pub(super) struct CatalogFilter {
    source: CatalogSourceFilter,
    kind: Option<String>,
    query: String,
}

pub(super) fn catalog_location_label(
    location: &vapor_studio_core::CatalogLocation,
) -> &'static str {
    match location.kind {
        vapor_studio_core::CatalogLocationKind::Workspace => "Workspace",
        vapor_studio_core::CatalogLocationKind::Cache => "Vapor cache",
        vapor_studio_core::CatalogLocationKind::Installed if location.enabled => "Installed",
        vapor_studio_core::CatalogLocationKind::Installed => "Installed · disabled",
    }
}

impl CatalogFilter {
    pub(super) fn new(source: CatalogSourceFilter, kind: Option<String>, query: &str) -> Self {
        Self {
            source,
            kind,
            query: query.trim().to_lowercase(),
        }
    }

    pub(super) fn matches_catalog(&self, content: &vapor_studio_core::CatalogContent) -> bool {
        let source_matches =
            match self.source {
                CatalogSourceFilter::All => true,
                CatalogSourceFilter::Workspace => content.locations.iter().any(|location| {
                    location.kind == vapor_studio_core::CatalogLocationKind::Workspace
                }),
                CatalogSourceFilter::Cache => content
                    .locations
                    .iter()
                    .any(|location| location.kind == vapor_studio_core::CatalogLocationKind::Cache),
                CatalogSourceFilter::Installed => content.locations.iter().any(|location| {
                    location.kind == vapor_studio_core::CatalogLocationKind::Installed
                }),
                CatalogSourceFilter::Workshop => content.published_file_id.is_some(),
            };
        if !source_matches
            || self
                .kind
                .as_deref()
                .is_some_and(|kind| content.kind != kind)
        {
            return false;
        }
        self.query.is_empty()
            || content.id.to_lowercase().contains(&self.query)
            || content.kind.to_lowercase().contains(&self.query)
            || content
                .version
                .as_deref()
                .unwrap_or_default()
                .to_lowercase()
                .contains(&self.query)
            || content
                .published_file_id
                .as_deref()
                .unwrap_or_default()
                .contains(&self.query)
    }

    pub(super) fn matches_workspace(
        &self,
        candidate: &vapor_studio_core::UnregisteredProjectCandidate,
    ) -> bool {
        if !matches!(
            self.source,
            CatalogSourceFilter::All | CatalogSourceFilter::Workspace
        ) {
            return false;
        }
        let kind = candidate.kind.to_string();
        if self.kind.as_deref().is_some_and(|filter| filter != kind) {
            return false;
        }
        self.query.is_empty()
            || candidate.id.to_lowercase().contains(&self.query)
            || kind.to_lowercase().contains(&self.query)
            || candidate
                .path
                .to_string_lossy()
                .to_lowercase()
                .contains(&self.query)
    }
}

fn service_catalog_entry(entry: &ServiceCatalogEntry) -> vapor_studio_core::CatalogContent {
    vapor_studio_core::CatalogContent {
        id: entry.id.clone(),
        version: entry.version.clone(),
        kind: entry.kind.clone(),
        published_file_id: entry.published_file_id.clone(),
        dependencies: entry.dependencies.clone(),
        conflicts: entry
            .conflicts
            .iter()
            .map(|conflict| vapor_studio_core::CatalogConflict {
                field: conflict.field.clone(),
                first_value: conflict.first_value.clone(),
                first_location: PathBuf::from(&conflict.first_location),
                conflicting_value: conflict.conflicting_value.clone(),
                conflicting_location: PathBuf::from(&conflict.conflicting_location),
            })
            .collect(),
        locations: entry
            .locations
            .iter()
            .map(|location| vapor_studio_core::CatalogLocation {
                kind: match location.kind {
                    ServiceLocationKind::Workspace => {
                        vapor_studio_core::CatalogLocationKind::Workspace
                    }
                    ServiceLocationKind::Cache => vapor_studio_core::CatalogLocationKind::Cache,
                    ServiceLocationKind::Installed => {
                        vapor_studio_core::CatalogLocationKind::Installed
                    }
                },
                root: PathBuf::from(&location.root),
                enabled: location.enabled,
                fingerprint: location.fingerprint.clone(),
                install_source: location.install_source.clone(),
            })
            .collect(),
    }
}

fn service_source_project(
    project: &vapor_studio_protocol::WorkspaceSourceProject,
) -> vapor_studio_core::SourceContent {
    vapor_studio_core::SourceContent {
        id: project.id.clone(),
        version: project.version.clone(),
        kind: project.kind.clone(),
        root: PathBuf::from(&project.root),
        manifest: PathBuf::from(&project.manifest),
        binaries: project.binaries.clone(),
        libraries: project.libraries.clone(),
        dependencies: project.dependencies.clone(),
        workshop: vapor_studio_core::SourceWorkshopPolicy {
            app_id: project.workshop.app_id,
            published_file_id: project.workshop.published_file_id.clone(),
            visibility: project.workshop.visibility.clone(),
            title: project.workshop.title.clone(),
            description: project.workshop.description.clone(),
            tags: project.workshop.tags.clone(),
            change_note: project.workshop.change_note.clone(),
        },
    }
}

fn service_workspace_candidate(
    candidate: &vapor_studio_protocol::WorkspaceProjectCandidate,
) -> Option<vapor_studio_core::UnregisteredProjectCandidate> {
    Some(vapor_studio_core::UnregisteredProjectCandidate {
        path: PathBuf::from(&candidate.path),
        id: candidate.id.clone(),
        kind: vapor_studio_core::source_project_kind(&candidate.kind)?,
    })
}

#[cfg(test)]
mod selector_tests {
    use super::{content_selector_matches, service_source_project, service_workspace_candidate};
    use vapor_studio_protocol::{
        WorkspaceProjectCandidate, WorkspaceSourceProject, WorkspaceWorkshopPolicy,
    };

    #[test]
    fn service_workspace_projection_preserves_source_intent_and_candidate_identity() {
        let source = service_source_project(&WorkspaceSourceProject {
            id: "example.game".into(),
            version: Some("1.2.3".into()),
            kind: "game".into(),
            root: "/work/game".into(),
            manifest: "/work/game/Vapor.toml".into(),
            binaries: vec!["example-game".into()],
            libraries: vec![],
            dependencies: vec!["required · example.engine".into()],
            workshop: WorkspaceWorkshopPolicy {
                app_id: Some(2122620),
                published_file_id: Some("12345".into()),
                visibility: Some("public".into()),
                title: Some("Example game".into()),
                description: Some("A test fixture".into()),
                tags: vec!["game".into()],
                change_note: Some("Initial release".into()),
            },
        });
        assert_eq!(source.id, "example.game");
        assert_eq!(source.version.as_deref(), Some("1.2.3"));
        assert_eq!(source.workshop.app_id, Some(2122620));
        assert_eq!(source.workshop.published_file_id.as_deref(), Some("12345"));

        let candidate = service_workspace_candidate(&WorkspaceProjectCandidate {
            path: "/work/mods/terrain".into(),
            id: "terrain".into(),
            kind: "game-mod".into(),
        })
        .expect("known Rust-defined content kind");
        assert_eq!(
            candidate.path,
            std::path::PathBuf::from("/work/mods/terrain")
        );
        assert_eq!(candidate.id, "terrain");
        assert_eq!(candidate.kind.to_string(), "game-mod");
    }

    #[test]
    fn unsupported_workspace_candidate_kind_is_not_misrepresented() {
        let candidate = service_workspace_candidate(&WorkspaceProjectCandidate {
            path: "mods/future".into(),
            id: "future".into(),
            kind: "future-kind".into(),
        });
        assert!(candidate.is_none());
    }

    #[test]
    fn service_selection_preserves_exact_content_version() {
        assert!(content_selector_matches(
            "moon",
            Some("1.2.0"),
            "moon@1.2.0"
        ));
        assert!(!content_selector_matches(
            "moon",
            Some("1.2.0"),
            "moon@1.3.0"
        ));
        assert!(!content_selector_matches(
            "moon",
            Some("1.2.0"),
            "moon-extra@1.2.0"
        ));
    }

    #[test]
    fn unversioned_content_matches_only_its_exact_identity() {
        assert!(content_selector_matches("moon", None, "moon"));
        assert!(!content_selector_matches("moon", None, "moon@1.2.0"));
    }
}
