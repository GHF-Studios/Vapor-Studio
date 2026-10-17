use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

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
        graph
            .effective_game
            .as_ref()
            .map(identity)
            .unwrap_or_default()
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

/// Check that an installed graph still satisfies the identities saved by a profile.
/// Missing legacy payload/source facts are tolerated once so a successful launch can
/// upgrade that profile to a complete lock. Facts already present must match exactly.
pub fn resolved_graph_matches(
    expected: &ResolvedCompositionGraph,
    actual: &ResolvedCompositionGraph,
) -> bool {
    if expected.root != actual.root
        || expected.effective_engine != actual.effective_engine
        || expected.effective_game != actual.effective_game
    {
        return false;
    }
    let Some(expected_nodes) = canonical_resolved_nodes(expected) else {
        return false;
    };
    let Some(actual_nodes) = canonical_resolved_nodes(actual) else {
        return false;
    };
    if expected_nodes.len() != actual_nodes.len() {
        return false;
    }
    expected_nodes.iter().all(|(identity, expected)| {
        let Some(actual) = actual_nodes.get(identity) else {
            return false;
        };
        expected.kind == actual.kind
            && expected.dependencies == actual.dependencies
            && expected
                .fingerprint
                .is_none_or(|fingerprint| actual.fingerprint == Some(fingerprint))
            && expected
                .source
                .is_none_or(|source| actual.source == Some(source))
    })
}

struct CanonicalResolvedNode<'a> {
    kind: &'a str,
    fingerprint: Option<&'a str>,
    source: Option<&'a str>,
    dependencies: Vec<(String, ContentIdentity)>,
}

fn canonical_resolved_nodes(
    graph: &ResolvedCompositionGraph,
) -> Option<BTreeMap<ContentIdentity, CanonicalResolvedNode<'_>>> {
    let mut nodes = BTreeMap::new();
    for node in &graph.nodes {
        let mut bindings = BTreeSet::new();
        let mut dependencies = node
            .dependencies
            .iter()
            .map(|dependency| {
                if !bindings.insert(dependency.binding.clone()) {
                    return None;
                }
                Some((dependency.binding.clone(), dependency.content.clone()))
            })
            .collect::<Option<Vec<_>>>()?;
        dependencies.sort();
        if nodes
            .insert(
                node.content.clone(),
                CanonicalResolvedNode {
                    kind: &node.kind,
                    fingerprint: node.fingerprint.as_deref(),
                    source: node.source.as_deref(),
                    dependencies,
                },
            )
            .is_some()
        {
            return None;
        }
    }
    if !nodes.contains_key(&graph.root)
        || !nodes.contains_key(&graph.effective_engine)
        || graph
            .effective_game
            .as_ref()
            .is_some_and(|game| !nodes.contains_key(game))
        || nodes.values().any(|node| {
            node.dependencies
                .iter()
                .any(|(_, dependency)| !nodes.contains_key(dependency))
        })
    {
        return None;
    }
    Some(nodes)
}

#[cfg(test)]
mod resolved_graph_tests {
    use super::{
        ContentIdentity, InstallationState, ResolvedCompositionGraph, ResolvedDependencyIdentity,
        ResolvedNodeIdentity, resolved_graph_matches,
    };

    fn identity(id: &str, version: &str) -> ContentIdentity {
        ContentIdentity {
            id: id.to_owned(),
            version: Some(version.to_owned()),
        }
    }

    fn graph(engine_fingerprint: &str) -> ResolvedCompositionGraph {
        ResolvedCompositionGraph {
            root: identity("org.game", "1.0.0"),
            effective_engine: identity("org.engine", "2.0.0"),
            effective_game: Some(identity("org.gameplay", "3.0.0")),
            nodes: vec![
                ResolvedNodeIdentity {
                    content: identity("org.engine", "2.0.0"),
                    kind: "Engine".to_owned(),
                    fingerprint: Some(engine_fingerprint.to_owned()),
                    source: Some("workshop:42".to_owned()),
                    dependencies: Vec::new(),
                },
                ResolvedNodeIdentity {
                    content: identity("org.game", "1.0.0"),
                    kind: "Packagepack".to_owned(),
                    fingerprint: Some("sha256:root".to_owned()),
                    source: Some("workspace:/game".to_owned()),
                    dependencies: vec![
                        ResolvedDependencyIdentity {
                            binding: "engine".to_owned(),
                            content: identity("org.engine", "2.0.0"),
                        },
                        ResolvedDependencyIdentity {
                            binding: "game".to_owned(),
                            content: identity("org.gameplay", "3.0.0"),
                        },
                    ],
                },
                ResolvedNodeIdentity {
                    content: identity("org.gameplay", "3.0.0"),
                    kind: "Game".to_owned(),
                    fingerprint: Some("sha256:game".to_owned()),
                    source: Some("cache".to_owned()),
                    dependencies: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn resolved_graph_match_ignores_record_order_but_checks_payload_and_edges() {
        let expected = graph("sha256:engine");
        let mut reordered = expected.clone();
        reordered.nodes.reverse();
        reordered.nodes[1].dependencies.reverse();
        assert!(resolved_graph_matches(&expected, &reordered));

        let changed_payload = graph("sha256:changed");
        assert!(!resolved_graph_matches(&expected, &changed_payload));

        let mut changed_source = expected.clone();
        changed_source.nodes[0].source = Some("cache".to_owned());
        assert!(!resolved_graph_matches(&expected, &changed_source));

        let mut changed_engine_version = expected.clone();
        changed_engine_version.effective_engine.version = Some("2.0.1".to_owned());
        assert!(!resolved_graph_matches(&expected, &changed_engine_version));

        let mut changed_edges = expected.clone();
        changed_edges.nodes[1].dependencies[1].content = identity("org.other-game", "3.0.0");
        assert!(!resolved_graph_matches(&expected, &changed_edges));
        assert!(!resolved_graph_matches(&changed_edges, &changed_edges));

        let mut duplicate_binding = expected.clone();
        duplicate_binding.nodes[1].dependencies[1].binding = "engine".to_owned();
        assert!(!resolved_graph_matches(
            &duplicate_binding,
            &duplicate_binding
        ));
    }

    #[test]
    fn installed_file_verification_does_not_survive_profile_reload() {
        let state = InstallationState::OutOfDate {
            detail: "graph changed".to_owned(),
            actual_graph: graph("sha256:engine"),
            artifacts: Vec::new(),
            verified: true,
        };
        let encoded = serde_json::to_string(&state).unwrap();
        assert!(!encoded.contains("verified"));
        let decoded: InstallationState = serde_json::from_str(&encoded).unwrap();
        assert!(matches!(
            decoded,
            InstallationState::OutOfDate {
                verified: false,
                ..
            }
        ));
    }

    #[test]
    fn legacy_graph_without_provenance_can_be_upgraded_after_identity_match() {
        let actual = graph("sha256:engine");
        let mut legacy = actual.clone();
        for node in &mut legacy.nodes {
            node.fingerprint = None;
            node.source = None;
        }
        assert!(resolved_graph_matches(&legacy, &actual));

        legacy.nodes[0]
            .dependencies
            .push(ResolvedDependencyIdentity {
                binding: "asset".to_owned(),
                content: identity("org.asset", "1.0.0"),
            });
        assert!(!resolved_graph_matches(&legacy, &actual));
    }
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
    OutOfDate {
        detail: String,
        actual_graph: ResolvedCompositionGraph,
        artifacts: Vec<InstalledArtifactRecord>,
        /// Session-local proof; installed files must be checked again after restart.
        #[serde(skip)]
        verified: bool,
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

/// Rename one profile while preserving case-insensitive name uniqueness.
pub fn rename_profile(
    profiles: &mut [CompositionProfile],
    index: usize,
    name: &str,
) -> Result<(), String> {
    if index >= profiles.len() {
        return Err("The selected profile no longer exists.".to_owned());
    }
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a non-empty profile name.".to_owned());
    }
    if profiles
        .iter()
        .enumerate()
        .any(|(other_index, other)| other_index != index && other.name.eq_ignore_ascii_case(name))
    {
        return Err("A profile with that name already exists.".to_owned());
    }
    profiles[index].name = name.to_owned();
    Ok(())
}

/// Remove one profile without allowing a profile set to become empty.
pub fn remove_profile(
    profiles: &mut Vec<CompositionProfile>,
    index: usize,
) -> Result<CompositionProfile, String> {
    if profiles.len() <= 1 {
        return Err("Studio must keep at least one profile.".to_owned());
    }
    if index >= profiles.len() {
        return Err("The selected profile no longer exists.".to_owned());
    }
    Ok(profiles.remove(index))
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
