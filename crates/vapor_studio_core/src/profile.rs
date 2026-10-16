use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Stable identity for the root content selected by a launchable profile.
/// A missing version means the profile was migrated from an older ID-only selection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ContentIdentity {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedDependencyIdentity {
    pub binding: String,
    pub content: ContentIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedNodeIdentity {
    pub content: ContentIdentity,
    pub kind: String,
    /// Fingerprint of the exact installed payload represented by this node.
    /// Older saved graphs have no payload lock and deserialize as `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// Source recorded by Vapor for the installed artifact, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub dependencies: Vec<ResolvedDependencyIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedCompositionGraph {
    pub root: ContentIdentity,
    pub effective_engine: ContentIdentity,
    #[serde(default)]
    pub effective_game: Option<ContentIdentity>,
    pub nodes: Vec<ResolvedNodeIdentity>,
}

/// Stable fingerprint for an installed graph with exact payload and source
/// records for every node. Returns `None` for legacy or partial snapshots.
pub fn fingerprint_resolved_graph(graph: &ResolvedCompositionGraph) -> Option<String> {
    if graph.nodes.is_empty()
        || graph.nodes.iter().any(|node| {
            node.fingerprint.is_none() || node.source.as_deref().map_or(true, str::is_empty)
        })
    {
        return None;
    }
    let identity = |content: &ContentIdentity| {
        let version = content.version.as_deref().map_or_else(
            || "0".to_owned(),
            |version| format!("1{}:{}", version.len(), version),
        );
        format!("{}:{}{version}", content.id.len(), content.id)
    };
    let mut canonical = String::new();
    canonical.push_str(&format!("root={}\n", identity(&graph.root)));
    canonical.push_str(&format!("engine={}\n", identity(&graph.effective_engine)));
    canonical.push_str(&format!(
        "game={}\n",
        graph.effective_game.as_ref().map(identity).unwrap_or_default()
    ));
    let nodes: BTreeMap<_, _> = graph
        .nodes
        .iter()
        .map(|node| (identity(&node.content), node))
        .collect();
    if nodes.len() != graph.nodes.len() {
        return None;
    }
    for (content, node) in nodes {
        canonical.push_str(&format!("node={content}\n"));
        canonical.push_str(&format!("kind={}:{}\n", node.kind.len(), node.kind));
        let source = node.source.as_deref().unwrap_or_default();
        canonical.push_str(&format!("source={}:{}\n", source.len(), source));
        let fingerprint = node.fingerprint.as_deref().unwrap_or_default();
        canonical.push_str(&format!("payload={}:{}\n", fingerprint.len(), fingerprint));
        let mut dependencies = node
            .dependencies
            .iter()
            .map(|dependency| {
                let target = identity(&dependency.content);
                format!(
                    "{}:{}{}:{}",
                    dependency.binding.len(),
                    dependency.binding,
                    target.len(),
                    target
                )
            })
            .collect::<Vec<_>>();
        dependencies.sort();
        for dependency in dependencies {
            canonical.push_str(&format!("edge={content}{dependency}\n"));
        }
    }
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in canonical.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Some(format!("vapor-graph-fnv1a-64-v1:{hash:016x}"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceAssociation {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchSettings {
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledArtifactRecord {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
    pub root: PathBuf,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum InstallationState {
    #[default]
    Unknown,
    NotInstalled,
    Installed {
        artifacts: Vec<InstalledArtifactRecord>,
        graph_fingerprint: Option<String>,
    },
    Partial {
        detail: String,
    },
    NeedsRepair {
        detail: String,
        artifacts: Vec<InstalledArtifactRecord>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub root_package: Option<ContentIdentity>,
    #[serde(default)]
    pub resolved_graph: Option<ResolvedCompositionGraph>,
    #[serde(default)]
    pub workspace: Option<WorkspaceAssociation>,
    #[serde(default)]
    pub launch: LaunchSettings,
    /// Live installation observation; intentionally omitted from saved profiles.
    #[serde(skip)]
    pub installation: InstallationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDocument {
    pub schema_version: u32,
    pub profiles: Vec<CompositionProfile>,
}

impl Default for ProfileDocument {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            profiles: vec![CompositionProfile::default_profile()],
        }
    }
}

impl CompositionProfile {
    pub fn default_profile() -> Self {
        Self {
            id: "default".to_owned(),
            name: "Default".to_owned(),
            root_package: None,
            resolved_graph: None,
            workspace: None,
            launch: LaunchSettings::default(),
            installation: InstallationState::Unknown,
        }
    }
}

/// Add an empty composition profile associated with the current authoring workspace.
pub fn add_profile(
    profiles: &mut Vec<CompositionProfile>,
    name: &str,
    workspace: Option<PathBuf>,
) -> Result<usize, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a non-empty profile name.".to_owned());
    }
    if profiles
        .iter()
        .any(|profile| profile.name.eq_ignore_ascii_case(name))
    {
        return Err("A profile with that name already exists.".to_owned());
    }
    let id = next_profile_id(profiles);
    profiles.push(CompositionProfile {
        id,
        name: name.to_owned(),
        root_package: None,
        resolved_graph: None,
        workspace: workspace.map(|root| WorkspaceAssociation { root }),
        launch: LaunchSettings::default(),
        installation: InstallationState::Unknown,
    });
    Ok(profiles.len() - 1)
}

/// Duplicate a composition's root, resolved graph, workspace, and launch settings.
/// Installed artifacts remain app-owned and are therefore shared by the copy.
pub fn duplicate_profile(
    profiles: &mut Vec<CompositionProfile>,
    index: usize,
) -> Result<usize, String> {
    let source = profiles
        .get(index)
        .cloned()
        .ok_or_else(|| "The selected profile no longer exists.".to_owned())?;
    let mut copy = source.clone();
    copy.id = next_profile_id(profiles);
    let base_name = format!("{} Copy", source.name);
    copy.name = unique_profile_name(profiles, &base_name);
    profiles.push(copy);
    Ok(profiles.len() - 1)
}

fn next_profile_id(profiles: &[CompositionProfile]) -> String {
    (1_u64..)
        .map(|index| format!("profile-{index}"))
        .find(|candidate| profiles.iter().all(|profile| profile.id != *candidate))
        .expect("profile ID space exhausted")
}

fn unique_profile_name(profiles: &[CompositionProfile], base: &str) -> String {
    if profiles
        .iter()
        .all(|profile| !profile.name.eq_ignore_ascii_case(base))
    {
        return base.to_owned();
    }
    (2_u64..)
        .map(|index| format!("{base} {index}"))
        .find(|candidate| {
            profiles
                .iter()
                .all(|profile| !profile.name.eq_ignore_ascii_case(candidate))
        })
        .expect("profile name space exhausted")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRecovery {
    pub message: String,
}

/// Load the versioned document, or migrate the former eframe `Vec<Profile>`
/// shape. A malformed value recovers to a usable default profile with a visible diagnostic.
pub fn decode_profiles(value: Option<&str>) -> (ProfileDocument, Option<ProfileRecovery>) {
    let Some(value) = value else {
        return (ProfileDocument::default(), None);
    };
    if let Ok(document) = serde_json::from_str::<ProfileDocument>(value) {
        if document.schema_version == CURRENT_SCHEMA_VERSION && !document.profiles.is_empty() {
            return (document, None);
        }
        return (
            ProfileDocument::default(),
            Some(ProfileRecovery {
                message: format!(
                    "Saved Studio profile schema {} is unsupported or empty; a default profile was opened. The saved value was left untouched in eframe storage.",
                    document.schema_version
                ),
            }),
        );
    }
    #[derive(Deserialize)]
    struct LegacyProfile {
        name: String,
        #[serde(default)]
        selected_content: Option<String>,
        #[serde(default)]
        workspace_root: Option<PathBuf>,
    }
    if let Ok(legacy) = serde_json::from_str::<Vec<LegacyProfile>>(value)
        && !legacy.is_empty()
    {
        let profiles = legacy
            .into_iter()
            .enumerate()
            .map(|(index, old)| CompositionProfile {
                id: format!("profile-{}", index + 1),
                name: old.name,
                root_package: old
                    .selected_content
                    .map(|id| ContentIdentity { id, version: None }),
                resolved_graph: None,
                workspace: old.workspace_root.map(|root| WorkspaceAssociation { root }),
                launch: LaunchSettings::default(),
                installation: InstallationState::Unknown,
            })
            .collect();
        return (ProfileDocument { schema_version: CURRENT_SCHEMA_VERSION, profiles }, Some(ProfileRecovery {
            message: "Migrated existing Studio profiles. Their selected content IDs were retained; exact versions and installation state will be resolved from the content source.".to_owned(),
        }));
    }
    (ProfileDocument::default(), Some(ProfileRecovery {
        message: "Saved Studio profiles could not be read; a default profile was opened. The saved value was left untouched in eframe storage.".to_owned(),
    }))
}
