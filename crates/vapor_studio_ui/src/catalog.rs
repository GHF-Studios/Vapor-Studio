use super::StudioApp;
use super::operation_protocol::OperationResult;
use crate::navigation::CatalogSourceFilter;
use std::path::{Path, PathBuf};
use vapor_core::ContentKind;
use vapor_studio_core::ContentIdentity;
use vapor_studio_protocol::{
    CatalogEntry as ServiceCatalogEntry, CatalogLocationKind as ServiceLocationKind, ProtocolFrame,
    StudioAction, StudioEvent, StudioResponseKind,
};

pub(super) struct WorkspaceDiscovery {
    pub(super) source: Result<Vec<vapor_studio_core::SourceContent>, String>,
    pub(super) unregistered: Result<vapor_studio_core::UnregisteredProjectScan, String>,
    pub(super) validation_error: Option<String>,
}

pub(super) fn discover_workspace_data(
    executable: &Path,
    workspace_root: Option<&Path>,
) -> WorkspaceDiscovery {
    let workspace_validation_error = workspace_root.and_then(|root| {
        vapor_core::VaporWorkspace::load(root)
            .err()
            .map(|error| format!("{}: {error}", root.display()))
    });
    let source = workspace_root
        .map(|root| vapor_studio_core::discover_source_content(executable, root))
        .unwrap_or_else(|| Ok(Vec::new()));
    let unregistered = workspace_root
        .map(|root| vapor_studio_core::scan_unregistered_content_projects(executable, root))
        .unwrap_or_else(|| Ok(vapor_studio_core::UnregisteredProjectScan::default()));
    WorkspaceDiscovery {
        source,
        unregistered,
        validation_error: workspace_validation_error,
    }
}

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
                    match response.error {
                        Some(error) => {
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
            self.catalog.catalog_discovery_error = Some(message.clone());
            self.service_error = Some(message);
            self.service = None;
        }
    }

    fn apply_service_catalog_snapshot(&mut self, snapshot: &vapor_studio_protocol::StudioSnapshot) {
        self.catalog.items = snapshot.catalog.iter().map(service_catalog_entry).collect();
        self.catalog.catalog_discovery_loaded = true;
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
    }

    pub(super) fn refresh_content_catalog(&mut self) {
        self.profile_composition.invalidate_preview();
        self.submit_service_action(StudioAction::Catalog(
            vapor_studio_protocol::CatalogAction::Refresh,
        ));
        let root = self
            .profile
            .items
            .get(self.profile.active_index)
            .and_then(|profile| profile.workspace.as_ref())
            .map(|workspace| workspace.root.clone());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Catalog refresh requested; workspace scan failed: {error}");
                return;
            }
        };
        self.spawn(
            vapor_studio_core::OperationKind::DiscoverContentProjects,
            move |_context| {
                let discovery = discover_workspace_data(&executable, root.as_deref());
                OperationResult::Discovery(
                    discovery.source,
                    discovery.unregistered,
                    discovery.validation_error,
                )
            },
        );
    }

    pub(super) fn apply_workspace_discovery(
        &mut self,
        source_result: Result<Vec<vapor_studio_core::SourceContent>, String>,
        unregistered_result: Result<vapor_studio_core::UnregisteredProjectScan, String>,
        workspace_validation_error: Option<String>,
    ) {
        self.workspace.workspace_validation_error = workspace_validation_error;
        match source_result {
            Ok(contents) => {
                self.catalog.sources = contents;
                self.catalog.source_discovery_loaded = true;
                self.catalog.source_discovery_error = None;
            }
            Err(error) => self.catalog.source_discovery_error = Some(error),
        }
        match unregistered_result {
            Ok(scan) => {
                self.workspace.unregistered_scan_completed = true;
                self.workspace.unregistered_scan_error = None;
                self.workspace.unregistered_projects = scan;
            }
            Err(error) => self.workspace.unregistered_scan_error = Some(error),
        }
        let source_count = self.catalog.sources.len();
        self.status = match self.catalog.source_discovery_error.as_deref() {
            None if self.profile.items[self.profile.active_index].workspace.is_some() => {
                format!("Discovered {source_count} workspace projects; the local catalog is provided by the Studio service.")
            }
            None => "Workspace source discovery is ready; the local catalog is provided by the Studio service.".to_owned(),
            Some(error) => format!("Workspace source discovery failed: {error}"),
        };
        if let Some(error) = self.workspace.unregistered_scan_error.as_deref() {
            self.status.push_str(&format!(
                " Workspace project scan could not complete: {error}"
            ));
        }
        if self.workspace.unregistered_scan_completed {
            let candidates = self.workspace.unregistered_projects.candidates.len();
            if candidates > 0 {
                self.status.push_str(&format!(
                    " Found {candidates} existing project{} you can add to Studio.",
                    if candidates == 1 { "" } else { "s" },
                ));
            }
        }
        if let Some(error) = self.workspace.workspace_validation_error.as_deref() {
            self.status.push_str(&format!(
                " Workspace validation needs repair; save fixes and refresh workspace content before running workspace operations. {error}"
            ));
        }
        if self.catalog.local_catalog_current() && self.catalog.source_snapshot_current() {
            match self.profile_composition.take_repreview_request() {
                Some(super::composition_lifecycle::RepreviewAfterCatalogRefresh::ActiveProfile) => {
                    self.preview_selected_composition();
                }
                Some(super::composition_lifecycle::RepreviewAfterCatalogRefresh::Source {
                    selector,
                    workspace_root,
                }) => {
                    self.preview_composition(
                        selector,
                        Some(workspace_root),
                        "Re-resolving the source composition against the refreshed cache…",
                    );
                }
                None => {}
            }
        }
        if self.catalog.selected.as_ref().is_some_and(|identity| {
            !self
                .catalog
                .items
                .iter()
                .any(|content| catalog_content_selector_matches(content, identity))
        }) {
            self.catalog.selected = None;
        }
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

#[cfg(test)]
mod selector_tests {
    use super::content_selector_matches;

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
