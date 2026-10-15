#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    Home,
    Discover,
    Library,
    Create,
    Workspace,
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
        Self::All, Self::Workspace, Self::Cache, Self::Installed, Self::Workshop,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "All local sources",
            Self::Workspace => "Workspace source",
            Self::Cache => "Vapor cache",
            Self::Installed => "Installed",
            Self::Workshop => "Workshop-backed",
        }
    }
}

impl Section {
    pub(super) const ALL: [Self; 5] = [
        Self::Home,
        Self::Discover,
        Self::Library,
        Self::Create,
        Self::Workspace,
    ];
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Discover => "Discover",
            Self::Library => "Library",
            Self::Create => "Create",
            Self::Workspace => "Workspace",
            Self::Diagnostics => "Diagnostics",
        }
    }
}
