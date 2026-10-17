//! Stable, typed messages exchanged between Vapor Studio clients and its service.
//!
//! This crate contains wire data only. It has no process, UI, or Vapor authority
//! dependencies so both sides can evolve behind an explicit protocol version.

mod action;
mod message;
mod state;

pub use action::{
    AuthoringAction, CatalogAction, CompositionAction, DeveloperAction, ProfileAction,
    PublishingAction, StudioAction, WorkshopAction, WorkspaceAction,
};
pub use message::{
    PROTOCOL_VERSION, ProtocolFrame, RequestId, StudioError, StudioErrorCode, StudioRequest,
    StudioResponse, StudioResponseKind,
};
pub use state::{
    AvailableAction, CatalogConflict, CatalogEntry, CatalogLocation, CatalogLocationKind,
    HostSummary, OperationEvent, OperationPhase, OperationStatus, OperationSummary, StudioEvent,
    StudioRevision, StudioSnapshot,
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
            StudioAction::Composition(CompositionAction::Preview),
            StudioAction::Composition(CompositionAction::Install),
            StudioAction::Composition(CompositionAction::Verify),
            StudioAction::Composition(CompositionAction::Repair),
            StudioAction::Composition(CompositionAction::Update),
            StudioAction::Composition(CompositionAction::Launch),
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
            51,
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
        assert_round_trip(StudioEvent::Snapshot(StudioSnapshot {
            revision: StudioRevision(3),
            host: HostSummary {
                app_root: Some("/app".into()),
                workspace: None,
                mode: "production".into(),
            },
            active_profile_id: None,
            active_operation: Some(OperationSummary {
                id: "operation-1".into(),
                action: "catalog.refresh".into(),
                phase: OperationPhase::Resolving,
                status: OperationStatus::Running,
                progress: None,
            }),
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
            selected_content_id: None,
            available_actions: vec![AvailableAction {
                id: "catalog.refresh".into(),
                enabled: true,
                reason: None,
            }],
        }));
        assert_round_trip(StudioEvent::OperationChanged(OperationEvent {
            revision: StudioRevision(4),
            id: "operation-1".into(),
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
}
