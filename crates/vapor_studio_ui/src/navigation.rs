#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Section {
    Home,
    Discover,
    Library,
    Compose,
    Create,
    Workspace,
    Publish,
    Diagnostics,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum CatalogSourceFilter {
    #[default]
    All,
    Workspace,
    Cache,
    Installed,
    Workshop,
}

impl CatalogSourceFilter {
    pub(super) const ALL: [Self; 5] = [
        Self::All,
        Self::Workspace,
        Self::Cache,
        Self::Installed,
        Self::Workshop,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "All on this device",
            Self::Workspace => "Project folders",
            Self::Cache => "Downloaded here",
            Self::Installed => "Ready to play",
            Self::Workshop => "From Steam Workshop",
        }
    }
}

impl Section {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Home => "Play",
            Self::Discover => "Browse",
            Self::Library => "Profiles",
            Self::Compose => "Game setup",
            Self::Create => "Create",
            Self::Workspace => "Projects",
            Self::Publish => "Share",
            Self::Diagnostics => "App status",
        }
    }
}
