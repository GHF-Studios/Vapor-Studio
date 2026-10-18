//! Stable, typed messages exchanged between Vapor Studio clients and its service.
//!
//! This crate contains wire data only. It has no process, UI, or Vapor authority
//! dependencies so both sides can evolve behind an explicit protocol version.

mod action;
mod message;
mod profile;
mod state;

pub use action::{
    AuthoringAction, CatalogAction, CompositionAction, DeveloperAction, ProfileAction,
    PublishingAction, SessionAction, StudioAction, WorkshopAction, WorkspaceAction,
};
pub use message::{
    PROTOCOL_VERSION, ProtocolFrame, RequestId, StudioError, StudioErrorCode, StudioRequest,
    StudioResponse, StudioResponseKind,
};
pub use profile::{
    CompositionProfile, ContentIdentity, InstallationState, InstalledArtifactRecord,
    LaunchSettings, LegacyStorageSeed, PROFILE_SCHEMA_VERSION, ProfileDocument,
    ResolvedCompositionGraph, ResolvedDependencyIdentity, ResolvedNodeIdentity,
    WorkspaceAssociation,
};
pub use state::{
    AvailableAction, CatalogConflict, CatalogEntry, CatalogLocation, CatalogLocationKind,
    CompositionPreviewDependency, CompositionPreviewNode, CompositionPreviewSnapshot,
    CompositionPreviewSource, DurableStateSnapshot, HostSummary, OperationEvent, OperationPhase,
    OperationStatus, OperationSummary, StudioEvent, StudioRevision, StudioSnapshot,
    WorkspaceContentSnapshot, WorkspaceProjectCandidate, WorkspaceSourceProject,
    WorkspaceWorkshopPolicy,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_round_trip<T>(value: T)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug + PartialEq,
    {
        let encoded = serde_json::to_string(&value).expect("encode protocol value");
        let decoded: T = serde_json::from_str(&encoded).expect("decode protocol value");
        assert_eq!(decoded, value);
    }

    #[test]
    fn every_semantic_action_round_trips() {
        let actions = vec![
            StudioAction::Profile(ProfileAction::Create {
                name: "Test".into(),
            }),
            StudioAction::Profile(ProfileAction::Duplicate {
                profile_id: "p1".into(),
                name: "Copy".into(),
            }),
            StudioAction::Profile(ProfileAction::Rename {
                profile_id: "p1".into(),
                name: "Renamed".into(),
            }),
            StudioAction::Profile(ProfileAction::Remove {
                profile_id: "p1".into(),
            }),
            StudioAction::Profile(ProfileAction::Select {
                profile_id: "p1".into(),
            }),
            StudioAction::Profile(ProfileAction::SetRoot {
                profile_id: "p1".into(),
                content_id: Some("root".into()),
                content_version: Some("1.0.0".into()),
                workspace_root: Some("/workspace".into()),
            }),
            StudioAction::Profile(ProfileAction::ResolveDefaultRoot {
                profile_id: "p1".into(),
            }),
            StudioAction::Profile(ProfileAction::SetLaunchArguments {
                profile_id: "p1".into(),
                arguments: vec!["--safe".into()],
            }),
            StudioAction::Profile(ProfileAction::AdoptVerifiedGraph {
                profile_id: "p1".into(),
            }),
            StudioAction::Profile(ProfileAction::InvalidateContent {
                content_id: "root".into(),
                content_version: Some("1.0.0".into()),
            }),
            StudioAction::Session(SessionAction::CheckpointLocalState {
                editor_drafts: Some("{}".into()),
                operation_history: Some("[]".into()),
            }),
            StudioAction::Catalog(CatalogAction::Refresh),
            StudioAction::Catalog(CatalogAction::DiscoverContentProjects),
            StudioAction::Catalog(CatalogAction::Search {
                query: "moon".into(),
            }),
            StudioAction::Catalog(CatalogAction::Inspect {
                content_id: "moon".into(),
                version: Some("1.0.0".into()),
            }),
            StudioAction::Catalog(CatalogAction::AdoptWorkspaceProject {
                path: "mods/moon".into(),
            }),
            StudioAction::Composition(CompositionAction::Preview {
                profile_id: "p1".into(),
                selector: Some("pack".into()),
            }),
            StudioAction::Composition(CompositionAction::Install {
                profile_id: "p1".into(),
            }),
            StudioAction::Composition(CompositionAction::Verify {
                profile_id: "p1".into(),
            }),
            StudioAction::Composition(CompositionAction::Repair {
                profile_id: "p1".into(),
            }),
            StudioAction::Composition(CompositionAction::Update {
                profile_id: "p1".into(),
            }),
            StudioAction::Composition(CompositionAction::Launch {
                profile_id: "p1".into(),
            }),
            StudioAction::Composition(CompositionAction::RunSource {
                profile_id: "p1".into(),
            }),
            StudioAction::Workspace(WorkspaceAction::Discover { root: ".".into() }),
            StudioAction::Workspace(WorkspaceAction::DiscoverWorkspace { root: ".".into() }),
            StudioAction::Workspace(WorkspaceAction::Open { root: ".".into() }),
            StudioAction::Workspace(WorkspaceAction::Validate),
            StudioAction::Workspace(WorkspaceAction::Remember),
            StudioAction::Workspace(WorkspaceAction::Create {
                root: "/tmp".into(),
                name: "Demo".into(),
            }),
            StudioAction::Workspace(WorkspaceAction::AdoptProject {
                path: "mods/moon".into(),
            }),
            StudioAction::Workspace(WorkspaceAction::UnregisterProject {
                project_id: "moon".into(),
            }),
            StudioAction::Workspace(WorkspaceAction::ListFiles { path: ".".into() }),
            StudioAction::Workspace(WorkspaceAction::OpenFile {
                path: "Vapor.toml".into(),
            }),
            StudioAction::Workspace(WorkspaceAction::SaveFile {
                path: "Vapor.toml".into(),
                expected_fingerprint: "abc".into(),
                contents: "[project]".into(),
            }),
            StudioAction::Workspace(WorkspaceAction::CreateFile {
                path: "new.toml".into(),
                contents: String::new(),
            }),
            StudioAction::Authoring(AuthoringAction::LoadComposition {
                project_id: "moon".into(),
            }),
            StudioAction::Authoring(AuthoringAction::SaveComposition {
                project_id: "moon".into(),
                expected_fingerprint: "abc".into(),
                dependencies: vec![],
            }),
            StudioAction::Authoring(AuthoringAction::LoadWorkshopMetadata {
                project_id: "moon".into(),
            }),
            StudioAction::Authoring(AuthoringAction::SaveWorkshopMetadata {
                project_id: "moon".into(),
                expected_fingerprint: "abc".into(),
                title: "Moon".into(),
                tags: vec![],
            }),
            StudioAction::Authoring(AuthoringAction::LoadRuntimeOutputs {
                project_id: "moon".into(),
            }),
            StudioAction::Authoring(AuthoringAction::SaveRuntimeOutputs {
                project_id: "moon".into(),
                expected_fingerprint: "abc".into(),
                binaries: vec![],
                libraries: vec![],
            }),
            StudioAction::Authoring(AuthoringAction::CreateContent {
                workspace_root: ".".into(),
                kind: "mod".into(),
                id: "moon".into(),
                version: "1.0.0".into(),
            }),
            StudioAction::Authoring(AuthoringAction::CreateWorkspace {
                root: "/tmp".into(),
                name: "Demo".into(),
            }),
            StudioAction::Workshop(WorkshopAction::Search {
                query: "moon".into(),
                page: 1,
            }),
            StudioAction::Workshop(WorkshopAction::LoadSubscriptions),
            StudioAction::Workshop(WorkshopAction::Subscribe {
                item_ids: vec!["123".into()],
            }),
            StudioAction::Workshop(WorkshopAction::Unsubscribe {
                item_ids: vec!["123".into()],
            }),
            StudioAction::Workshop(WorkshopAction::Acquire {
                item_ids: vec!["123".into()],
            }),
            StudioAction::Workshop(WorkshopAction::Stage {
                project_id: "moon".into(),
            }),
            StudioAction::Workshop(WorkshopAction::Publish {
                review_id: "review-1".into(),
                confirmation: "confirm".into(),
            }),
            StudioAction::Publishing(PublishingAction::StageRoot),
            StudioAction::Publishing(PublishingAction::PublishRoot {
                review_id: "root-review".into(),
                confirmation: "confirm".into(),
            }),
            StudioAction::Developer(DeveloperAction::Format),
            StudioAction::Developer(DeveloperAction::Check),
            StudioAction::Developer(DeveloperAction::Test),
            StudioAction::Developer(DeveloperAction::Build),
            StudioAction::Developer(DeveloperAction::Clean {
                confirmation: "clean-1".into(),
            }),
        ];
        assert_eq!(
            actions.len(),
            57,
            "update this catalog when adding/removing action variants"
        );
        for action in actions {
            assert_round_trip(action);
        }
    }

    #[test]
    fn request_event_snapshot_and_error_round_trip() {
        assert_round_trip(StudioRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: RequestId("request-1".into()),
            expected_revision: Some(StudioRevision(3)),
            action: StudioAction::Catalog(CatalogAction::Refresh),
        });
        assert_round_trip(StudioEvent::Snapshot(Box::new(StudioSnapshot {
            revision: StudioRevision(3),
            host: HostSummary {
                app_root: Some("/app".into()),
                workspace: None,
                mode: "production".into(),
                platform: "linux".into(),
                steam_client_available: true,
            },
            active_profile_id: None,
            durable_state: DurableStateSnapshot::default(),
            active_operation: Some(OperationSummary {
                id: "operation-1".into(),
                profile_id: Some("p1".into()),
                action: "catalog.refresh".into(),
                phase: OperationPhase::Resolving,
                status: OperationStatus::Running,
                progress: None,
            }),
            composition_preview: None,
            catalog: vec![CatalogEntry {
                id: "moon".into(),
                version: Some("1.0.0".into()),
                kind: "mod".into(),
                published_file_id: None,
                dependencies: vec![],
                conflicts: vec![],
                locations: vec![CatalogLocation {
                    kind: CatalogLocationKind::Workspace,
                    root: "mods/moon".into(),
                    enabled: true,
                    fingerprint: None,
                    install_source: None,
                }],
            }],
            workspace_content: WorkspaceContentSnapshot {
                source_projects: vec![WorkspaceSourceProject {
                    id: "loo-cast.game".into(),
                    version: Some("1.0.0".into()),
                    kind: "game".into(),
                    root: "game".into(),
                    manifest: "game/Vapor.toml".into(),
                    binaries: vec!["loo-cast-game".into()],
                    libraries: vec![],
                    dependencies: vec!["required · spacetime-engine".into()],
                    workshop: WorkspaceWorkshopPolicy {
                        app_id: Some(2122620),
                        published_file_id: Some("3762162407".into()),
                        visibility: Some("public".into()),
                        title: Some("Loo-Cast Game".into()),
                        description: Some("Game content".into()),
                        tags: vec!["game".into()],
                        change_note: None,
                    },
                }],
                source_projects_loaded: true,
                source_projects_error: None,
                unregistered_projects: vec![WorkspaceProjectCandidate {
                    path: "mods/terrain".into(),
                    id: "mod-terrain".into(),
                    kind: "game-mod".into(),
                }],
                unregistered_diagnostics: vec!["ignored invalid project".into()],
                unregistered_projects_loaded: true,
                unregistered_projects_error: None,
                workspace_validation_error: Some("metadata needs repair".into()),
            },
            selected_content_id: None,
            available_actions: vec![AvailableAction {
                id: "catalog.refresh".into(),
                enabled: true,
                reason: None,
            }],
        })));
        assert_round_trip(StudioEvent::OperationChanged(OperationEvent {
            revision: StudioRevision(4),
            id: "operation-1".into(),
            profile_id: Some("p1".into()),
            action: "catalog.refresh".into(),
            phase: OperationPhase::Complete,
            status: OperationStatus::Succeeded,
            progress: Some(1.0),
            message: Some("Done".into()),
        }));
        assert_round_trip(StudioError {
            code: StudioErrorCode::StaleRevision,
            message: "state changed".into(),
            retryable: true,
        });
        assert_round_trip(ProtocolFrame::Event(StudioEvent::ServiceReady {
            protocol_version: PROTOCOL_VERSION,
        }));
    }

    #[test]
    fn snapshots_from_the_previous_protocol_shape_default_workspace_discovery() {
        let prior_snapshot = r#"{
            "revision": 0,
            "host": {"app_root": null, "workspace": null, "mode": "production"},
            "active_profile_id": null,
            "active_operation": null,
            "catalog": [],
            "selected_content_id": null,
            "available_actions": []
        }"#;

        let snapshot: StudioSnapshot = serde_json::from_str(prior_snapshot)
            .expect("older snapshot omitting additive workspace state remains readable");
        assert_eq!(
            snapshot.workspace_content,
            WorkspaceContentSnapshot::default()
        );
    }

    #[test]
    fn profile_runtime_installation_state_round_trips_over_the_protocol() {
        let mut profile = CompositionProfile::default_profile();
        profile.installation = InstallationState::NotInstalled;
        assert_round_trip(ProfileDocument {
            schema_version: PROFILE_SCHEMA_VERSION,
            profiles: vec![profile],
        });
    }
}
