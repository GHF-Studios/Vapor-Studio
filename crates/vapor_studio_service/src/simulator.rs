use crate::scenario::load_scenario;
use std::{
    io::{self, BufRead, Write},
    path::Path,
    time::Duration,
};
use vapor_studio_client::{ServiceBootstrap as ClientBootstrap, StudioClient};
use vapor_studio_protocol::{
    CatalogAction, ProfileAction, ProtocolFrame, StudioAction, WorkshopAction,
};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

pub fn run_simulator(
    service_executable: &Path,
    scenario_path: Option<&Path>,
) -> Result<(), String> {
    if let Some(path) = scenario_path {
        return run_scenario(service_executable, path);
    }
    run_interactive(service_executable)
}

fn spawn_fixture_client(executable: &Path) -> Result<StudioClient, String> {
    StudioClient::spawn(
        executable,
        ClientBootstrap {
            application_executable: executable.to_path_buf(),
            workspace_root: None,
            active_profile_id: Some("default".to_owned()),
            fixture_mode: true,
            legacy_storage: vapor_studio_protocol::LegacyStorageSeed::default(),
            state_directory: None,
        },
    )
    .map_err(|error| error.to_string())
}

fn run_scenario(executable: &Path, scenario_path: &Path) -> Result<(), String> {
    let scenario = load_scenario(scenario_path)?;
    let mut client = spawn_fixture_client(executable)?;
    println!("Scenario: {}", scenario.name);
    println!("Service: fixture mode; no production provider is available.");
    for (index, step) in scenario.steps.into_iter().enumerate() {
        let previous_revision = client.latest_snapshot().revision;
        let request_id = client
            .send(step.action.clone(), Some(previous_revision))
            .map_err(|error| error.to_string())?;
        let frame = client
            .recv_timeout(RESPONSE_TIMEOUT)
            .map_err(|error| error.to_string())?;
        let ProtocolFrame::Response(response) = frame else {
            return Err(format!("step {} received a non-response frame", index + 1));
        };
        if response.request_id != request_id {
            return Err(format!(
                "step {} received a mismatched request ID",
                index + 1
            ));
        }
        if step
            .expect
            .response
            .is_some_and(|expected| expected != response.kind)
        {
            return Err(format!(
                "step {} expected {:?}, got {:?}",
                index + 1,
                step.expect.response,
                response.kind
            ));
        }
        if step.expect.error.is_some_and(|expected| {
            response.error.as_ref().map(|error| error.code) != Some(expected)
        }) {
            return Err(format!(
                "step {} expected error {:?}, got {:?}",
                index + 1,
                step.expect.error,
                response.error
            ));
        }
        if step
            .expect
            .active_profile_id
            .as_ref()
            .is_some_and(|expected| response.snapshot.active_profile_id.as_ref() != Some(expected))
        {
            return Err(format!(
                "step {} active profile did not match expectation",
                index + 1
            ));
        }
        if step
            .expect
            .selected_content_id
            .as_ref()
            .is_some_and(|expected| {
                response.snapshot.selected_content_id.as_ref() != Some(expected)
            })
        {
            return Err(format!(
                "step {} selected content did not match expectation",
                index + 1
            ));
        }
        if step
            .expect
            .catalog_len
            .is_some_and(|expected| response.snapshot.catalog.len() != expected)
        {
            return Err(format!(
                "step {} expected {} catalog entries, got {}",
                index + 1,
                step.expect.catalog_len.unwrap_or_default(),
                response.snapshot.catalog.len()
            ));
        }
        if step
            .expect
            .workspace_source_projects_len
            .is_some_and(|expected| {
                response.snapshot.workspace_content.source_projects.len() != expected
            })
        {
            return Err(format!(
                "step {} expected {} workspace source projects, got {}",
                index + 1,
                step.expect
                    .workspace_source_projects_len
                    .unwrap_or_default(),
                response.snapshot.workspace_content.source_projects.len()
            ));
        }
        if step
            .expect
            .workspace_candidates_len
            .is_some_and(|expected| {
                response
                    .snapshot
                    .workspace_content
                    .unregistered_projects
                    .len()
                    != expected
            })
        {
            return Err(format!(
                "step {} expected {} unregistered projects, got {}",
                index + 1,
                step.expect.workspace_candidates_len.unwrap_or_default(),
                response
                    .snapshot
                    .workspace_content
                    .unregistered_projects
                    .len()
            ));
        }
        println!(
            "{}. {} · revision {} → {} · {:?}{}",
            index + 1,
            step.action.id(),
            previous_revision.0,
            response.revision.0,
            response.kind,
            response
                .error
                .as_ref()
                .map(|error| format!(" · {}", error.message))
                .unwrap_or_default(),
        );
    }
    client
        .shutdown(Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    println!("Scenario passed.");
    Ok(())
}

fn run_interactive(executable: &Path) -> Result<(), String> {
    let mut client = spawn_fixture_client(executable)?;
    println!("Vapor Studio service simulator · fixture mode");
    print_snapshot(client.latest_snapshot());
    println!("Type help for commands; this process cannot publish to Workshop or Steam.");
    let stdin = io::stdin();
    let mut input = String::new();
    loop {
        print!("studio> ");
        io::stdout().flush().map_err(|error| error.to_string())?;
        input.clear();
        if stdin
            .lock()
            .read_line(&mut input)
            .map_err(|error| error.to_string())?
            == 0
        {
            break;
        }
        let line = input.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let action = match parts.next().unwrap_or_default() {
            "help" => {
                println!(
                    "open WORKSPACE | discover | refresh | search QUERY | inspect ID [VERSION] | profile ID | publish | snapshot | quit"
                );
                continue;
            }
            "refresh" => StudioAction::Catalog(CatalogAction::Refresh),
            "discover" => StudioAction::Catalog(CatalogAction::DiscoverContentProjects),
            "open" => {
                let Some(root) = parts.next() else {
                    println!("usage: open WORKSPACE_PATH");
                    continue;
                };
                StudioAction::Workspace(vapor_studio_protocol::WorkspaceAction::Open {
                    root: root.to_owned(),
                })
            }
            "search" => StudioAction::Catalog(CatalogAction::Search {
                query: parts.collect::<Vec<_>>().join(" "),
            }),
            "inspect" => {
                let Some(content_id) = parts.next() else {
                    println!("usage: inspect ID [VERSION]");
                    continue;
                };
                StudioAction::Catalog(CatalogAction::Inspect {
                    content_id: content_id.to_owned(),
                    version: parts.next().map(ToOwned::to_owned),
                })
            }
            "profile" => {
                let Some(profile_id) = parts.next() else {
                    println!("usage: profile ID");
                    continue;
                };
                StudioAction::Profile(ProfileAction::Select {
                    profile_id: profile_id.to_owned(),
                })
            }
            "publish" => StudioAction::Workshop(WorkshopAction::Publish {
                review_id: "simulated-review".into(),
                confirmation: "simulated-confirmation".into(),
            }),
            "snapshot" => {
                print_snapshot(client.latest_snapshot());
                continue;
            }
            "quit" | "exit" => break,
            command => {
                println!("unknown command: {command}");
                continue;
            }
        };
        let revision = client.latest_snapshot().revision;
        client
            .send(action, Some(revision))
            .map_err(|error| error.to_string())?;
        match client
            .recv_timeout(RESPONSE_TIMEOUT)
            .map_err(|error| error.to_string())?
        {
            ProtocolFrame::Response(response) => {
                println!(
                    "{:?} · revision {}{}",
                    response.kind,
                    response.revision.0,
                    response
                        .error
                        .map(|error| format!(" · {}", error.message))
                        .unwrap_or_default()
                );
                print_snapshot(&response.snapshot);
            }
            frame => println!("unexpected service frame: {frame:?}"),
        }
    }
    client
        .shutdown(Duration::from_secs(2))
        .map_err(|error| error.to_string())
}

fn print_snapshot(snapshot: &vapor_studio_protocol::StudioSnapshot) {
    println!(
        "revision {} · {} catalog item(s) · host mode {}",
        snapshot.revision.0,
        snapshot.catalog.len(),
        snapshot.host.mode
    );
    if !snapshot.workspace_content.source_projects.is_empty()
        || snapshot.workspace_content.source_projects_loaded
        || snapshot.workspace_content.unregistered_projects_loaded
    {
        println!(
            "workspace · {} source project(s) · {} adoption candidate(s){}{}",
            snapshot.workspace_content.source_projects.len(),
            snapshot.workspace_content.unregistered_projects.len(),
            snapshot
                .workspace_content
                .source_projects_error
                .as_ref()
                .map(|error| format!(" · source discovery error: {error}"))
                .unwrap_or_default(),
            snapshot
                .workspace_content
                .unregistered_projects_error
                .as_ref()
                .map(|error| format!(" · project scan error: {error}"))
                .unwrap_or_default(),
        );
    }
    for entry in &snapshot.catalog {
        println!(
            "  {} {} [{}] · {} location(s)",
            entry.id,
            entry.version.as_deref().unwrap_or(""),
            entry.kind,
            entry.locations.len()
        );
    }
    if let Some(profile) = &snapshot.active_profile_id {
        println!("active profile: {profile}");
    }
}
