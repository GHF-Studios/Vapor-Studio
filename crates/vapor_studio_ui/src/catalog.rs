use super::StudioApp;
use crate::navigation::CatalogSourceFilter;
use vapor_core::ContentKind;
use vapor_studio_core::ContentIdentity;

pub(super) struct CatalogState {
    pub(super) search_query: String,
    pub(super) sources: Vec<vapor_studio_core::SourceContent>,
    pub(super) items: Vec<vapor_studio_core::CatalogContent>,
    pub(super) source_filter: CatalogSourceFilter,
    pub(super) kind_filter: Option<String>,
    pub(super) selected: Option<String>,
}

impl Default for CatalogState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            sources: Vec::new(),
            items: Vec::new(),
            source_filter: CatalogSourceFilter::All,
            kind_filter: None,
            selected: None,
        }
    }
}

impl CatalogState {
    pub(super) fn clear_workspace_content(&mut self) {
        self.sources.clear();
        self.items.clear();
        self.selected = None;
    }
}

impl StudioApp {
    pub(super) fn apply_catalog_discovery(
        &mut self,
        source_result: Result<Vec<vapor_studio_core::SourceContent>, String>,
        catalog_result: Result<Vec<vapor_studio_core::CatalogContent>, String>,
    ) {
        let source_error = match source_result {
            Ok(contents) => {
                self.catalog.sources = contents;
                None
            }
            Err(error) => {
                self.catalog.sources.clear();
                Some(error)
            }
        };
        match catalog_result {
            Ok(catalog) => {
                let count = catalog.len();
                self.catalog.items = catalog;
                self.status = if self.profile.items[self.profile.active_index].workspace.is_some() {
                    source_error.map(|error| format!("Found {count} local items, but workspace sources could not load: {error}"))
                        .unwrap_or_else(|| format!("Discovered {count} local content identities across workspace, cache, and installation."))
                } else {
                    format!("Discovered {count} cached and installed content identities.")
                };
            }
            Err(error) => {
                self.catalog.items.clear();
                self.catalog.selected = None;
                self.status = source_error.map_or_else(
                    || format!("Could not refresh the local content catalog: {error}"),
                    |source_error| format!("Workspace sources failed: {source_error}. Local catalog failed: {error}"),
                );
            }
        }
        if self.profile_composition.take_repreview_request() {
            self.preview_selected_composition();
        }
        if self
            .catalog
            .selected
            .as_ref()
            .is_some_and(|identity| {
                !self
                    .catalog
                    .items
                    .iter()
                    .any(|content| catalog_content_selector(content) == *identity)
            })
        {
            self.catalog.selected = None;
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
