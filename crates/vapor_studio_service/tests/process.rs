use std::{path::PathBuf, time::Duration};
use vapor_studio_client::{ServiceBootstrap, StudioClient};
use vapor_studio_protocol::{
    CatalogAction, ProtocolFrame, StudioAction, StudioResponseKind, WorkshopAction,
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
        "workspace-open-recovery.json",
        "fixture-publisher-boundary.json",
    ] {
        run_simulator(&executable, Some(&scenarios.join(name)))
            .unwrap_or_else(|error| panic!("scenario {name} failed: {error}"));
    }
}
