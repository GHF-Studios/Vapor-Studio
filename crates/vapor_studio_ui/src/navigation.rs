#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Section {
    Home,
    Compositions,
    Discover,
    Library,
    Compose,
    Create,
    Workspace,
    Publish,
    Diagnostics,
}

pub(super) struct NavigationGroup {
    pub(super) label: &'static str,
    pub(super) sections: &'static [Section],
    pub(super) area: NavigationArea,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NavigationArea {
    Player,
    Creator,
    Application,
}

/// The canonical grouping and ordering used by both desktop and compact navigation.
pub(super) const NAVIGATION_GROUPS: &[NavigationGroup] = &[
    NavigationGroup {
        label: "PLAY",
        sections: &[Section::Home, Section::Compositions],
        area: NavigationArea::Player,
    },
    NavigationGroup {
        label: "FIND CONTENT",
        sections: &[Section::Discover, Section::Library],
        area: NavigationArea::Player,
    },
    NavigationGroup {
        label: "CREATOR TOOLS",
        sections: &[
            Section::Create,
            Section::Compose,
            Section::Workspace,
            Section::Publish,
        ],
        area: NavigationArea::Creator,
    },
    NavigationGroup {
        label: "APPLICATION",
        sections: &[Section::Diagnostics],
        area: NavigationArea::Application,
    },
];

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
            Self::All => "Available here",
            Self::Workspace => "In project folders",
            Self::Cache => "In your Library",
            Self::Installed => "Installed",
            Self::Workshop => "From Workshop",
        }
    }
}

impl Section {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Home => "Play",
            Self::Compositions => "Compositions",
            Self::Library => "Library",
            Self::Discover => "Browse",
            Self::Compose => "Compose",
            Self::Create => "Create",
            Self::Workspace => "Projects",
            Self::Publish => "Share",
            Self::Diagnostics => "App status",
        }
    }
}
