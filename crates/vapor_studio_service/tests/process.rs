use std::{fs, path::PathBuf, time::Duration};
use vapor_studio_client::{ServiceBootstrap, StudioClient};
use vapor_studio_protocol::{
    CatalogAction, LegacyStorageSeed, ProfileAction, ProfileDocument, ProtocolFrame, StudioAction,
    StudioResponseKind, WorkshopAction,
};
use vapor_studio_service::run_simulator;

fn service_executable() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_vapor-studio-service"))
}

fn fixture_client() -> StudioClient {
    let executable = service_executable();
    StudioClient::spawn(
        &executable,
        ServiceBootstrap {
            application_executable: executable.clone(),
            workspace_root: None,
            active_profile_id: Some("default".to_owned()),
            fixture_mode: true,
            legacy_storage: vapor_studio_protocol::LegacyStorageSeed::default(),
            state_directory: None,
        },
    )
    .expect("fixture service starts")
}

#[test]
fn child_process_returns_deterministic_catalog_snapshot_and_shutdowns() {
    let mut client = fixture_client();
    assert_eq!(client.latest_snapshot().host.mode, "fixture");
    assert_eq!(
        client.latest_snapshot().active_profile_id.as_deref(),
        Some("default")
    );
    let revision = client.latest_snapshot().revision;
    client
        .send(
            StudioAction::Catalog(CatalogAction::Refresh),
            Some(revision),
        )
        .expect("send refresh");
    let response = client
        .recv_timeout(Duration::from_secs(2))
        .expect("receive response");
    let ProtocolFrame::Response(response) = response else {
        panic!("expected a response frame");
    };
    assert_eq!(response.kind, StudioResponseKind::Applied);
    assert_eq!(response.snapshot.catalog.len(), 1);
    assert_eq!(response.snapshot.catalog[0].id, "fixture.engine");
    client
        .shutdown(Duration::from_secs(2))
        .expect("child shuts down");
}

#[test]
fn fixture_process_cannot_publish_to_workshop() {
    let mut client = fixture_client();
    let revision = client.latest_snapshot().revision;
    client
        .send(
            StudioAction::Workshop(WorkshopAction::Publish {
                review_id: "fixture-review".into(),
                confirmation: "fixture-confirmation".into(),
            }),
            Some(revision),
        )
        .expect("send publish attempt");
    let response = client
        .recv_timeout(Duration::from_secs(2))
        .expect("receive rejection");
    let ProtocolFrame::Response(response) = response else {
        panic!("expected a response frame");
    };
    assert_eq!(response.kind, StudioResponseKind::Rejected);
    assert_eq!(
        response.error.expect("structured rejection").code,
        vapor_studio_protocol::StudioErrorCode::ActionUnavailable
    );
}

#[test]
fn every_checked_in_fixture_scenario_runs_through_a_child_process() {
    let executable = service_executable();
    let scenarios = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scenarios");
    for name in [
        "fixture-profile-selection.json",
        "fixture-workspace-discovery.json",
        "workspace-open-recovery.json",
        "fixture-publisher-boundary.json",
    ] {
        run_simulator(&executable, Some(&scenarios.join(name)))
            .unwrap_or_else(|error| panic!("scenario {name} failed: {error}"));
    }
}

#[test]
fn production_service_imports_legacy_state_then_uses_its_store_after_restart() {
    let executable = service_executable();
    let state_directory = std::env::temp_dir().join(format!(
        "vapor-studio-restart-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let legacy_profiles = ProfileDocument::default();
    let legacy_storage = LegacyStorageSeed {
        profiles: Some(serde_json::to_string(&legacy_profiles).expect("encode legacy profiles")),
        active_profile_id: Some("default".into()),
        ..LegacyStorageSeed::default()
    };
    let mut client = StudioClient::spawn(
        &executable,
        ServiceBootstrap {
            application_executable: executable.clone(),
            workspace_root: None,
            active_profile_id: None,
            fixture_mode: false,
            legacy_storage,
            state_directory: Some(state_directory.clone()),
        },
    )
    .expect("production service imports legacy state");
    assert_eq!(
        client
            .latest_snapshot()
            .durable_state
            .profiles
            .profiles
            .len(),
        1
    );
    client
        .send(
            StudioAction::Profile(ProfileAction::Create {
                name: "Persisted profile".into(),
            }),
            Some(client.latest_snapshot().revision),
        )
        .expect("send profile creation");
    let response = client
        .recv_timeout(Duration::from_secs(2))
        .expect("receive profile creation");
    let ProtocolFrame::Response(response) = response else {
        panic!("expected profile response");
    };
    assert_eq!(response.kind, StudioResponseKind::Applied);
    assert_eq!(response.snapshot.durable_state.profiles.profiles.len(), 2);
    client
        .send(
            StudioAction::Profile(ProfileAction::SetLaunchArguments {
                profile_id: "profile-1".into(),
                arguments: vec!["--safe-mode".into()],
            }),
            Some(response.revision),
        )
        .expect("send profile launch settings");
    let response = client
        .recv_timeout(Duration::from_secs(2))
        .expect("receive launch settings");
    let ProtocolFrame::Response(response) = response else {
        panic!("expected launch settings response");
    };
    assert_eq!(response.kind, StudioResponseKind::Applied);
    assert_eq!(response.snapshot.durable_state.profile_revision, 2);
    client
        .shutdown(Duration::from_secs(2))
        .expect("first service shuts down");

    let restarted = StudioClient::spawn(
        &executable,
        ServiceBootstrap {
            application_executable: executable.clone(),
            workspace_root: None,
            active_profile_id: None,
            fixture_mode: false,
            legacy_storage: LegacyStorageSeed {
                profiles: Some("invalid and stale fallback".into()),
                active_profile_id: Some("missing".into()),
                ..LegacyStorageSeed::default()
            },
            state_directory: Some(state_directory.clone()),
        },
    )
    .expect("service reloads authoritative state");
    assert_eq!(
        restarted.latest_snapshot().durable_state.profiles.profiles[1].name,
        "Persisted profile"
    );
    assert_eq!(
        restarted.latest_snapshot().durable_state.active_profile_id,
        "profile-1"
    );
    assert_eq!(
        restarted.latest_snapshot().durable_state.profiles.profiles[1]
            .launch
            .arguments,
        ["--safe-mode"]
    );
    assert!(state_directory.join("studio-state.json").is_file());
    restarted
        .shutdown(Duration::from_secs(2))
        .expect("restarted service shuts down");
    fs::remove_dir_all(state_directory).expect("remove test state");
}
