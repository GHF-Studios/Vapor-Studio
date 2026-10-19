use super::StudioApp;
use crate::navigation::CatalogSourceFilter;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use vapor_client_application::ContentReference;
use vapor_client_protocol::{
    CatalogEntry as ServiceCatalogEntry, CatalogLocationKind as ServiceLocationKind, ClientAction,
    ClientEvent, ClientResponseKind, ProtocolFrame,
};
use vapor_core::ContentKind;

pub(super) struct CatalogState {
    pub(super) discover_tab: DiscoverTab,
    pub(super) search_query: String,
    pub(super) library_search_query: String,
    pub(super) library_kind_filter: Option<String>,
    pub(super) sources: Vec<vapor_client_application::SourceContent>,
    pub(super) source_discovery_loaded: bool,
    pub(super) source_discovery_error: Option<String>,
    pub(super) items: Vec<vapor_client_application::CatalogContent>,
    pub(super) service_item_actions: std::collections::BTreeMap<
        (String, Option<String>),
        Vec<vapor_client_protocol::CatalogItemAction>,
    >,
    pub(super) service_item_roles: std::collections::BTreeMap<
        (String, Option<String>),
        vapor_client_protocol::CatalogItemRole,
    >,
    pub(super) library_policies: Vec<vapor_client_protocol::LibraryItemPolicy>,
    pub(super) fork_identity: Option<String>,
    pub(super) fork_name: String,
    pub(super) add_to_composition_target: Option<String>,
    pub(super) catalog_discovery_loaded: bool,
    pub(super) catalog_omitted: usize,
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
            library_search_query: String::new(),
            library_kind_filter: None,
            sources: Vec::new(),
            source_discovery_loaded: false,
            source_discovery_error: None,
            items: Vec::new(),
            service_item_actions: std::collections::BTreeMap::new(),
            service_item_roles: std::collections::BTreeMap::new(),
            library_policies: Vec::new(),
            fork_identity: None,
            fork_name: String::new(),
            add_to_composition_target: None,
            catalog_discovery_loaded: false,
            catalog_omitted: 0,
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
        self.catalog_discovery_loaded
            && self.catalog_discovery_error.is_none()
            && self.catalog_omitted == 0
    }

    pub(super) fn clear_workspace_content(&mut self) {
        self.sources.clear();
        self.source_discovery_loaded = false;
        self.source_discovery_error = None;
        self.items.clear();
        self.service_item_actions.clear();
        self.service_item_roles.clear();
        self.fork_identity = None;
        self.fork_name.clear();
        self.add_to_composition_target = None;
        self.catalog_discovery_loaded = false;
        self.catalog_omitted = 0;
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
        self.catalog.fork_identity = None;
        self.catalog.fork_name.clear();
        self.submit_service_action(ClientAction::Catalog(
            vapor_client_protocol::CatalogAction::Inspect {
                content_id,
                version,
            },
        ));
    }

    pub(super) fn apply_service_action(&mut self, action: ClientAction) -> Result<(), String> {
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
                    return if response.kind == ClientResponseKind::Applied {
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
                ProtocolFrame::Event(ClientEvent::Snapshot(snapshot)) => {
                    self.apply_service_catalog_snapshot(&snapshot);
                }
                ProtocolFrame::Event(ClientEvent::OperationChanged(event)) => {
                    self.apply_service_operation_event(&event);
                }
                ProtocolFrame::Event(ClientEvent::WorkspaceFileResult(result)) => {
                    self.apply_service_workspace_file_result(*result);
                }
                ProtocolFrame::Event(ClientEvent::AuthoringResult(result)) => {
                    self.apply_service_authoring_result(*result);
                }
                ProtocolFrame::Event(ClientEvent::ProtocolError(error)) => {
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
        self.apply_service_action(ClientAction::Session(
            vapor_client_protocol::SessionAction::CheckpointLocalState {
                editor_drafts,
                operation_history,
            },
        ))
    }

    pub(super) fn submit_service_action(&mut self, action: ClientAction) {
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
                    let operation_request =
                        self.operation.service_request_id.as_ref() == Some(&response.request_id);
                    let operation_kind = self
                        .operation
                        .active_operation
                        .as_ref()
                        .map(|operation| operation.kind.clone());
                    if operation_request {
                        self.operation.service_request_id = None;
                        if response.kind != ClientResponseKind::OperationAccepted {
                            self.operation.running = false;
                            self.operation.service_owned = false;
                            self.operation.active_operation = None;
                        }
                    }
                    match response.error {
                        Some(error) => {
                            if operation_request {
                                match operation_kind {
                                    Some(vapor_client_application::OperationKind::SearchWorkshop) => {
                                        self.apply_workshop_error(error.message.clone());
                                    }
                                    Some(vapor_client_application::OperationKind::Acquire) => {
                                        self.apply_workshop_acquisition_error(error.message.clone());
                                    }
                                    Some(vapor_client_application::OperationKind::StageWorkshop) => {
                                        self.apply_workshop_stage_error(error.message.clone());
                                    }
                                    Some(vapor_client_application::OperationKind::PublishWorkshop) => {
                                        self.apply_workshop_error(error.message.clone());
                                    }
                                    Some(vapor_client_application::OperationKind::LoadWorkshopSubscriptions)
                                    | Some(vapor_client_application::OperationKind::ChangeWorkshopSubscription) => {
                                        self.apply_workshop_subscriptions_error(error.message.clone());
                                    }
                                    _ => self.status = error.message.clone(),
                                }
                            } else {
                                self.status = error.message;
                            }
                        }
                        None if !operation_request
                            && response.kind == ClientResponseKind::Applied =>
                        {
                            self.service_error = None;
                        }
                        None => {}
                    }
                }
                ProtocolFrame::Event(ClientEvent::Snapshot(snapshot)) => {
                    self.apply_service_catalog_snapshot(&snapshot);
                }
                ProtocolFrame::Event(ClientEvent::OperationChanged(event)) => {
                    self.apply_service_operation_event(&event);
                }
                ProtocolFrame::Event(ClientEvent::WorkspaceFileResult(result)) => {
                    self.apply_service_workspace_file_result(*result);
                }
                ProtocolFrame::Event(ClientEvent::AuthoringResult(result)) => {
                    self.apply_service_authoring_result(*result);
                }
                ProtocolFrame::Event(ClientEvent::ProtocolError(error)) => {
                    self.status = error.message.clone();
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
        snapshot: &vapor_client_protocol::ClientSnapshot,
    ) {
        self.workshop.steam_client_available = snapshot.host.steam_client_available;
        self.apply_service_workshop_snapshot(&snapshot.workshop);
        self.root_publishing.preview = snapshot.root_publishing.clone();
        if let Some(publishing) = snapshot.root_publishing.as_ref() {
            self.root_publishing.status = if publishing.planned {
                format!(
                    "Steam root review ready · App {} · depot {} · {} files · account {}.",
                    publishing.app_id,
                    publishing.depot_id,
                    publishing.copied_files,
                    publishing.account,
                )
            } else if publishing.succeeded {
                format!(
                    "Steam upload completed · App {} · depot {} · account {} · branch {}.",
                    publishing.app_id,
                    publishing.depot_id,
                    publishing.account,
                    publishing.set_live.as_deref().unwrap_or("not changed"),
                )
            } else {
                format!(
                    "Steam root upload finished with {}.",
                    publishing.status.as_deref().unwrap_or("unknown status"),
                )
            };
        }
        let durable = &snapshot.durable_state;
        self.catalog.library_policies = durable.library_policies.clone();
        if !durable.compositions.compositions.is_empty()
            && self.composition.service_revision != Some(durable.composition_revision)
        {
            self.composition.items = durable.compositions.compositions.clone();
            self.composition.active_index = self
                .composition
                .items
                .iter()
                .position(|composition| composition.id == durable.active_composition_id)
                .unwrap_or(0);
            self.composition.name_draft = self.composition.items[self.composition.active_index]
                .name
                .clone();
            self.composition.recovery_backup = durable.recovery_backup.clone();
            self.composition.service_revision = Some(durable.composition_revision);
        }
        let service_workspace = durable.authoring_workspace_root.clone().unwrap_or_default();
        if self.workspace.root != service_workspace && !self.workspace.editor.has_dirty_documents()
        {
            self.workspace.root = service_workspace.clone();
            self.workspace.root_draft = if service_workspace.is_empty() {
                self.workspace.fallback_root.display().to_string()
            } else {
                service_workspace
            };
        }
        self.catalog.service_item_actions = snapshot
            .catalog
            .iter()
            .map(|entry| {
                (
                    (entry.id.clone(), entry.version.clone()),
                    entry.actions.clone(),
                )
            })
            .collect();
        self.catalog.service_item_roles = snapshot
            .catalog
            .iter()
            .map(|entry| ((entry.id.clone(), entry.version.clone()), entry.role))
            .collect();
        self.catalog.items = snapshot.catalog.iter().map(service_catalog_entry).collect();
        self.catalog.catalog_discovery_loaded = snapshot.catalog_status.loaded;
        self.catalog.catalog_discovery_error = snapshot.catalog_status.error.clone();
        self.catalog.catalog_omitted = snapshot.catalog_status.omitted;
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
        self.workspace.unregistered_projects = vapor_client_application::UnregisteredProjectScan {
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
        let selected_composition_id = self
            .composition
            .items
            .get(self.composition.active_index)
            .map(|composition| composition.id.as_str());
        if let Some(preview) = snapshot
            .composition_preview
            .as_ref()
            .filter(|preview| Some(preview.composition_id.as_str()) == selected_composition_id)
        {
            let preview = protocol_composition_preview(preview);
            if self.composition_preview.preview() != Some(&preview) {
                self.apply_composition_preview(preview);
            }
        } else if snapshot.composition_preview.is_none()
            && self.composition_preview.preview().is_some()
        {
            self.composition_preview.invalidate_preview();
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
            match self.composition_preview.take_repreview_request() {
                Some(
                    super::composition_lifecycle::RepreviewAfterCatalogRefresh::ActiveComposition,
                ) => {
                    self.preview_selected_composition();
                }
                Some(super::composition_lifecycle::RepreviewAfterCatalogRefresh::Source {
                    selector,
                    workspace_root,
                }) => self.preview_composition(
                    selector,
                    Some(workspace_root),
                    "Refreshing available content for this composition…",
                ),
                None => {}
            }
        }
    }

    pub(super) fn refresh_content_catalog(&mut self) {
        self.composition_preview.invalidate_preview();
        self.submit_service_action(ClientAction::Catalog(
            vapor_client_protocol::CatalogAction::Refresh,
        ));
    }
}

fn protocol_composition_preview(
    preview: &vapor_client_protocol::CompositionPreviewSnapshot,
) -> vapor_client_application::CompositionPreview {
    vapor_client_application::CompositionPreview {
        selector: preview.selector.clone(),
        nodes: preview.nodes.clone(),
        blockers: preview.blockers.clone(),
        warnings: preview.warnings.clone(),
    }
}

pub(super) fn content_selector(identity: &ContentReference) -> String {
    identity
        .version
        .as_deref()
        .map(|version| format!("{}@{}", identity.id, version))
        .unwrap_or_else(|| identity.id.clone())
}

pub(crate) fn source_content_selector(content: &vapor_client_application::SourceContent) -> String {
    content
        .version
        .as_deref()
        .map(|version| format!("{}@{version}", content.id))
        .unwrap_or_else(|| content.id.clone())
}

pub(crate) fn source_content_selector_matches(
    content: &vapor_client_application::SourceContent,
    selector: &str,
) -> bool {
    content_selector_matches(&content.id, content.version.as_deref(), selector)
}

pub(crate) fn catalog_content_selector(
    content: &vapor_client_application::CatalogContent,
) -> String {
    content_selector(&content.identity())
}

pub(crate) fn catalog_content_selector_matches(
    content: &vapor_client_application::CatalogContent,
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
        "crate" => ContentKind::Crate,
        _ => return None,
    })
}

pub(crate) fn content_kind_name(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::Packagepack => "Packagepack",
        ContentKind::Enginepack => "Enginepack",
        ContentKind::Gamepack => "Gamepack",
        ContentKind::Modpack => "Modpack",
        ContentKind::Engine => "Engine",
        ContentKind::Game => "Game",
        ContentKind::EngineMod => "Engine Mod",
        ContentKind::GameMod => "Game Mod",
        ContentKind::ExtensionMod => "Extension Mod",
        ContentKind::Crate => "Crate",
    }
}

pub(crate) fn display_content_kind(kind: &str) -> String {
    source_content_kind(kind)
        .map(content_kind_name)
        .unwrap_or(kind)
        .to_owned()
}

/// Player-facing title for a catalog item. Stable IDs remain available in
/// technical details, while the authored name is used as the primary label.
pub(crate) fn display_catalog_name(content: &vapor_client_application::CatalogContent) -> String {
    display_content_name(&content.id, content.name.as_deref())
}

pub(crate) fn display_catalog_identity(
    id: &str,
    version: Option<&str>,
    catalog: &[vapor_client_application::CatalogContent],
) -> String {
    let name = catalog
        .iter()
        .find(|content| content.id == id && content.version.as_deref() == version)
        .map(display_catalog_name)
        .unwrap_or_else(|| display_content_name(id, None));
    version.map_or(name.clone(), |version| format!("{name} · v{version}"))
}

fn display_content_name(id: &str, authored_name: Option<&str>) -> String {
    authored_name
        .filter(|name| !name.trim().is_empty())
        .map(str::trim)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            let words = id
                .rsplit(['/', ':', '.'])
                .next()
                .unwrap_or(id)
                .split(['-', '_'])
                .filter(|word| !word.is_empty())
                .map(|word| {
                    let mut characters = word.chars();
                    characters.next().map_or_else(String::new, |first| {
                        first.to_uppercase().chain(characters).collect::<String>()
                    })
                })
                .collect::<Vec<_>>();
            if words.is_empty() {
                id.to_owned()
            } else {
                words.join(" ")
            }
        })
}

pub(super) struct CatalogFilter {
    source: CatalogSourceFilter,
    kind: Option<String>,
    query: vapor_client_protocol::CatalogQuery,
    library_only: bool,
}

pub(super) fn catalog_location_label(
    location: &vapor_client_application::CatalogLocation,
) -> &'static str {
    match location.kind {
        vapor_client_application::CatalogLocationKind::Workspace => "In a project",
        vapor_client_application::CatalogLocationKind::Cache => "In your Library",
        vapor_client_application::CatalogLocationKind::Installed if location.enabled => "Installed",
        vapor_client_application::CatalogLocationKind::Installed => "Installed · inactive",
    }
}

impl CatalogFilter {
    pub(super) fn new(source: CatalogSourceFilter, kind: Option<String>, query: &str) -> Self {
        Self {
            source,
            kind,
            query: vapor_client_protocol::CatalogQuery::new(query),
            library_only: false,
        }
    }

    pub(super) fn library(kind: Option<String>, query: &str) -> Self {
        Self {
            source: CatalogSourceFilter::All,
            kind,
            query: vapor_client_protocol::CatalogQuery::new(query),
            library_only: true,
        }
    }

    pub(super) fn matches_catalog(
        &self,
        content: &vapor_client_application::CatalogContent,
    ) -> bool {
        self.matches_catalog_with_role(content, None)
    }

    pub(super) fn matches_catalog_with_role(
        &self,
        content: &vapor_client_application::CatalogContent,
        role: Option<vapor_client_protocol::CatalogItemRole>,
    ) -> bool {
        let source_matches = match self.source {
            CatalogSourceFilter::All => true,
            CatalogSourceFilter::Workspace => content.locations.iter().any(|location| {
                location.kind == vapor_client_application::CatalogLocationKind::Workspace
            }),
            CatalogSourceFilter::Cache => content.locations.iter().any(|location| {
                location.kind == vapor_client_application::CatalogLocationKind::Cache
            }),
            CatalogSourceFilter::Installed => content.locations.iter().any(|location| {
                location.kind == vapor_client_application::CatalogLocationKind::Installed
            }),
            CatalogSourceFilter::Workshop => content.published_file_id.is_some(),
        };
        let library_matches = !self.library_only
            || (role.map_or_else(
                || {
                    content.kind != vapor_core::ContentKind::Packagepack.as_str()
                        && content.kind != vapor_core::ContentKind::Crate.as_str()
                },
                |role| role == vapor_client_protocol::CatalogItemRole::RuntimeContent,
            ) && content.locations.iter().any(|location| {
                matches!(
                    location.kind,
                    vapor_client_application::CatalogLocationKind::Cache
                        | vapor_client_application::CatalogLocationKind::Installed
                )
            }));
        if !source_matches
            || !library_matches
            || self
                .kind
                .as_deref()
                .is_some_and(|kind| content.kind != kind)
        {
            return false;
        }
        self.query.matches_fields([
            content.id.as_str(),
            content.name.as_deref().unwrap_or_default(),
            content.kind.as_str(),
            content.version.as_deref().unwrap_or_default(),
            content.published_file_id.as_deref().unwrap_or_default(),
        ])
    }

    pub(super) fn matches_workspace(
        &self,
        candidate: &vapor_client_application::UnregisteredProjectCandidate,
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
        self.query.matches_fields([
            candidate.id.as_str(),
            kind.as_str(),
            candidate.path.to_string_lossy().as_ref(),
        ])
    }
}

fn service_catalog_entry(entry: &ServiceCatalogEntry) -> vapor_client_application::CatalogContent {
    vapor_client_application::CatalogContent {
        id: entry.id.clone(),
        name: entry.name.clone(),
        version: entry.version.clone(),
        kind: entry.kind.clone(),
        published_file_id: entry.published_file_id.clone(),
        dependencies: entry.dependencies.clone(),
        conflicts: entry
            .conflicts
            .iter()
            .map(|conflict| vapor_client_application::CatalogConflict {
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
            .map(|location| vapor_client_application::CatalogLocation {
                kind: match location.kind {
                    ServiceLocationKind::Workspace => {
                        vapor_client_application::CatalogLocationKind::Workspace
                    }
                    ServiceLocationKind::Cache => {
                        vapor_client_application::CatalogLocationKind::Cache
                    }
                    ServiceLocationKind::Installed => {
                        vapor_client_application::CatalogLocationKind::Installed
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
    project: &vapor_client_protocol::WorkspaceSourceProject,
) -> vapor_client_application::SourceContent {
    vapor_client_application::SourceContent {
        id: project.id.clone(),
        version: project.version.clone(),
        kind: project.kind.clone(),
        root: PathBuf::from(&project.root),
        manifest: PathBuf::from(&project.manifest),
        binaries: project.binaries.clone(),
        libraries: project.libraries.clone(),
        dependencies: project.dependencies.clone(),
        workshop: vapor_client_application::SourceWorkshopPolicy {
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
    candidate: &vapor_client_protocol::WorkspaceProjectCandidate,
) -> Option<vapor_client_application::UnregisteredProjectCandidate> {
    Some(vapor_client_application::UnregisteredProjectCandidate {
        path: PathBuf::from(&candidate.path),
        id: candidate.id.clone(),
        kind: vapor_client_application::source_project_kind(&candidate.kind)?,
    })
}

#[cfg(test)]
mod selector_tests {
    use super::{content_selector_matches, service_source_project, service_workspace_candidate};
    use vapor_client_protocol::{
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

#[cfg(test)]
mod catalog_view_tests {
    use super::*;

    #[test]
    fn browse_and_library_queries_do_not_filter_each_others_facts() {
        let content = vapor_client_application::CatalogContent {
            id: "moon".into(),
            name: Some("Moon".into()),
            version: Some("1.0".into()),
            kind: "game-mod".into(),
            published_file_id: Some("3762162251".into()),
            dependencies: vec![],
            conflicts: vec![],
            locations: vec![vapor_client_application::CatalogLocation {
                kind: vapor_client_application::CatalogLocationKind::Cache,
                root: "download/moon".into(),
                enabled: true,
                fingerprint: None,
                install_source: None,
            }],
        };
        let browse = CatalogFilter::new(CatalogSourceFilter::All, None, "absent");
        let library = CatalogFilter::library(None, "3762162251");
        assert!(!browse.matches_catalog(&content));
        assert!(library.matches_catalog(&content));
        assert!(
            CatalogFilter::new(CatalogSourceFilter::All, None, " MOON ").matches_catalog(&content)
        );
    }
}
