# Vapor Studio Service Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Work natively in this session; do not delegate tasks.

**Goal:** Move every meaningful Studio workflow behind one short-lived service process so GUI and terminal clients exercise the same state transitions, operation lifecycle, and errors.

**Architecture:** Add typed protocol, application service, and client crates to Vapor-Studio. Package the service as a sibling executable beside vapor-gui; both the GUI and a developer terminal simulator spawn it over private stdio. The service owns workflow state and persistence, reuses existing Vapor authority operations, and accepts narrow production or deterministic test providers.

**Tech Stack:** Rust 2024, serde/serde_json, std::process, stdio JSON-lines, existing Vapor and Studio crates, Vapor-managed build and test commands.

**Spec:** [2026-10-17 Studio Service Boundary](../specs/2026-10-17-studio-service-boundary-design.md)

## Current checkpoint (2026-10-17)

Implemented and managed-validated: the typed protocol (51 semantic action
variants), child client/server lifecycle, fixture simulator, protocol and
process tests, and the first GUI adapter slice for local catalog refresh/search,
exact content selection, workspace activation, and profile selection. The
checked-in scenarios pass through the service child. `vapor validate --project
vapor-studio` passes all gates. This is an initial service foundation, not a
complete GUI migration; the detailed workflow tasks below remain open. A
`vapor root build --host-only` attempt stopped at `vapor-gui` because the
ignored local GUI lockfile is stale while managed builds enforce `--locked`, so
shared-root sibling promotion remains unverified. No deployment or launch was
performed.

## Global Constraints

- The service is a normal short-lived child process, not an OS service or persistent daemon.
- There is no network listener; clients use versioned typed messages over the child’s private standard streams.
- The service bootstraps from arguments and inherited environment once, resolves explicit host context, and passes it into operations.
- Business operations do not read ambient process environment directly.
- The service reuses Vapor authority and provider implementations; Studio does not duplicate Steam, Workshop, toolchain, content, installation, or publishing logic.
- The initial protocol supports one interactive client per service process.
- Session state is not silently persisted across process restarts; profiles, workspace settings, drafts, and history use an explicit durable store.
- Deterministic scenarios cannot publish or mutate a real Steam account.
- UI-only layout, focus, scrolling, and widget state remain with the GUI; semantic workflow state and action availability belong to the service.
- Do not add or rename content or pack types, redesign visual styling, or change Vapor’s authority boundaries.
- Run compilation and tests only through Vapor-managed commands from the Vapor-Client source context; do not invoke raw Cargo.
- Do not deploy, publish, or launch through Steam as part of this plan.

## Review Focus

- A client starts with a stale or incompatible protocol version; it must fail clearly and terminate the child without hanging.
- The child exits, emits malformed JSON, or closes one stream mid-operation; the client must report a bounded typed failure and remain recoverable.
- A second or stale action arrives while an operation is active or against an old state revision; action availability and rejection must be deterministic.
- A provider returns an error or partial acquisition; the service must preserve recoverable user state and report the precise outcome.
- A scenario requests Workshop publication; only a fixture may receive it, and no production Steam provider may be reachable from deterministic scenarios.

---

## File and crate map

- Vapor-Studio/crates/vapor_studio_protocol/: versioned action, snapshot, result, error, and event DTOs shared across process boundaries.
- Vapor-Studio/crates/vapor_studio_service/: session state machines, operation coordination, persistence, provider ports/adapters, stdio server, and scenario support.
- Vapor-Studio/crates/vapor_studio_client/: sibling-process lifecycle, framed transport, snapshot/event subscription, and typed client errors.
- Vapor-Studio/crates/vapor_studio_core/: existing domain values and Vapor operation adapters; retain only domain policy and authority calls, not UI workflow orchestration.
- Vapor-Studio/crates/vapor_studio_ui/: egui presentation and semantic-action mapping; replace its worker dispatcher and operation result reducer with the client and service snapshots.
- Vapor-Studio/crates/vapor_studio_service/src/main.rs: expose the private stdio server and developer simulator modes from the service executable.
- Vapor-Studio/Workspace.vapor.toml: promote vapor-studio-service into the same managed app-root binary directory as vapor-gui.
- .agents/workstreams/open/workshop-content-superpass.md: record service implementation checkpoints and preserve the active full-stack authoring workstream.

## Execution rules

- Keep the service API authoritative. UI and terminal clients send the same typed action and consume the same snapshot/event model.
- For each task, add the test first, run it to see the expected failure, implement the smallest coherent unit, then rerun the owning Vapor-managed suite.
- Commit a task only after its stated verification passes. Push task commits periodically after dependent crate revisions are pushed, then update and push the parent repository’s submodule pointers.
- Keep real Steam and filesystem effects behind production adapters. Tests inject fixtures; never use a real account or Workshop item in a scenario.
- Do not claim visual quality from service tests. Finish with an explicit human visual check of the GUI after the service-backed client is locally deployed, if deployment is later requested.

---

### Task 1: Define the complete semantic action and state contract

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_protocol/Cargo.toml
- Create: Vapor-Studio/crates/vapor_studio_protocol/src/lib.rs
- Create: Vapor-Studio/crates/vapor_studio_protocol/src/action.rs
- Create: Vapor-Studio/crates/vapor_studio_protocol/src/state.rs
- Create: Vapor-Studio/crates/vapor_studio_protocol/src/message.rs
**Interfaces:**
- Produces StudioAction, grouped into profile, catalog, composition, workspace, authoring, Workshop, publishing, and developer-operation actions.
- Produces StudioSnapshot, StudioRevision, StudioRequest, StudioResponse, StudioEvent, and structured StudioError.
- A request includes a correlation ID, supported protocol version, optional expected state revision, and one semantic action.
- A response distinguishes accepted asynchronous operations, completed synchronous actions, and rejected actions with the current snapshot.

**Steps:**
- [ ] Add protocol unit tests for JSON round trips of every action family, event, error, and snapshot.
- [ ] Run vapor test vapor-studio; confirm the protocol tests fail because the DTOs do not yet exist.
- [ ] Define serde-tagged, versioned message types with stable kebab-case discriminants and no untyped JSON action payloads.
- [ ] Add an exhaustive action-catalog test that represents every current user-triggered operation in vapor_studio_ui/src/operations.rs, composition_lifecycle.rs, workspace_ops.rs, workspace_content.rs, workspace_editor.rs, content_creation.rs, source_authoring.rs, workshop.rs, and root_publishing.rs.
- [ ] Rerun vapor test vapor-studio; confirm all protocol tests pass.
- [ ] Commit the protocol crate and managed workspace registration.

### Task 2: Build the service session model and reducer

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_service/Cargo.toml
- Create: Vapor-Studio/crates/vapor_studio_service/src/lib.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/session.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/state.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/availability.rs
**Interfaces:**
- StudioSession::new(context, providers, store) -> Result<StudioSession, StudioError>
- StudioSession::snapshot(&self) -> StudioSnapshot
- StudioSession::dispatch(&mut self, request: StudioRequest) -> StudioResponse
- StudioSession::apply_operation_event(&mut self, event: OperationEvent) -> Vec<StudioEvent>
- Action availability is computed from current service state and returned in the snapshot.

**Steps:**
- [ ] Add reducer tests for initial state, profile/workspace selection, busy-operation gating, stale revision rejection, and deterministic action availability.
- [ ] Run the Studio suite and confirm the new tests fail.
- [ ] Implement a single session owner with monotonic revisions; keep egui navigation, focus, scroll, and widget state out.
- [ ] Implement action dispatch as exhaustive matches over StudioAction; reject unknown semantic states with structured errors.
- [ ] Rerun vapor test vapor-studio; confirm reducer tests pass.
- [ ] Commit the session model.

### Task 3: Define provider ports and deterministic fixtures

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_service/src/providers/mod.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/providers/content.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/providers/workspace.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/providers/workshop.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/providers/development.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/providers/production.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/providers/fixtures.rs

**Interfaces:**
- StudioProviders composes narrow ContentProvider, WorkspaceProvider, WorkshopProvider, and DevelopmentProvider ports.
- Production adapters call existing vapor_studio_core, Vapor Core, Vapor Shell, and Steamworks integrations.
- Fixture adapters are constructed explicitly and cannot fall back to production providers.

**Steps:**
- [ ] Add fixture contract tests for successful operations, provider failures, partial Workshop acquisition, and publication interception.
- [ ] Confirm tests fail before provider traits exist.
- [ ] Extract only operation seams required by service actions; do not wrap unrelated Vapor APIs or create a generic plugin registry.
- [ ] Make production adapters the only constructor used by the production service host; require explicit fixture construction in tests and simulation mode.
- [ ] Add a test that attempts Workshop publication through fixtures and proves the production provider was not called.
- [ ] Rerun the Studio suite and commit provider ports and adapters.

### Task 4: Bootstrap host context and migrate durable state

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_service/src/host.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/persistence.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/legacy_import.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/app.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/app_shell.rs

**Interfaces:**
- StudioHostContext::resolve(args, inherited_environment) -> Result<StudioHostContext, StudioError> captures executable, app root, workspace, platform, and provider availability once.
- StudioStateStore loads and atomically saves versioned durable profiles, active profile ID, workspace association, editor drafts, and recent operation history.
- LegacyStorageSeed carries existing egui storage values into first service initialization; after successful migration, service storage is authoritative.

**Steps:**
- [ ] Add tests for explicit arguments overriding environment, absent optional environment values, secret redaction, malformed legacy profiles, and one-time migration.
- [ ] Confirm those tests fail before host and persistence types exist.
- [ ] Implement host resolution without environment reads in operation modules.
- [ ] Implement atomic versioned persistence under the platform user-data directory, retaining current profile/draft/history JSON semantics and importing current egui keys once.
- [ ] Verify restart round-trip and failed-write recovery with temporary directories.
- [ ] Rerun the Studio suite and commit host bootstrap and migration.

### Task 5: Move profiles, local discovery, and profile-root selection

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_service/src/session/profiles.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/session/catalog.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/profiles.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/catalog.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/command_palette.rs

**Interfaces:**
- Profile actions cover create, duplicate, rename, remove, select, root assignment, and default-root recovery.
- Catalog actions cover refresh, search/filter selection where it affects semantic selection, inspect, and workspace-project adoption.
- Snapshot projections preserve stale-vs-current discovery state and exact selected content identity.

**Steps:**
- [ ] Add tests for profile CRUD, active-profile invariants, last-profile protection, root role validation, duplicate catalog identity, stale snapshots, and ambiguous identities.
- [ ] Confirm tests fail before moving these transitions.
- [ ] Move profile and catalog transition logic into StudioSession; retain only widget drafts and rendering locally.
- [ ] Route command-palette operations through semantic actions, not private UI methods.
- [ ] Rerun tests and commit profile/catalog migration.

### Task 6: Move composition preview, install, verify, repair, update, and launch

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_service/src/session/composition.rs
- Modify: Vapor-Studio/crates/vapor_studio_core/src/shell_content.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/composition_lifecycle.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/operation_results.rs

**Interfaces:**
- Composition actions carry the selected profile ID and exact selector; the service resolves identity from authoritative profile state.
- Results distinguish preview blockers, installed graph, verification failures, launch handoff errors, and recovery results.
- Every post-install, post-repair, and post-launch catalog refresh is scheduled by service rules.

**Steps:**
- [ ] Add scenarios for missing dependencies, wrong root kind, installed graph mismatch, partial failure, successful repair, and launch executable failure.
- [ ] Confirm the new workflow tests fail before service transitions exist.
- [ ] Move lifecycle decisions and refresh rules into the service, continuing to call Vapor's existing preview/install/verify/repair/run operations.
- [ ] Ensure launch actions cannot report success when the runtime handoff returned an error.
- [ ] Rerun the Studio suite and commit composition lifecycle migration.

### Task 7: Move workspace lifecycle, editor, and development operations

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_service/src/session/workspace.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/session/editor.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/workspace_ops.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/workspace_content.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/workspace_editor.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/workspace_files.rs

**Interfaces:**
- Workspace actions cover open, validate, refresh, remember, create, adopt, unregister, and source selection.
- Editor actions carry canonical workspace-relative paths, expected file versions, and contents for guarded saves.
- Development operations carry the approved workspace root and operation kind; clean requires an explicit confirmation token bound to that root.

**Steps:**
- [ ] Add temporary-directory tests for workspace-open partial discovery, validation repair, file traversal rejection, stale-save conflict, multi-file partial save, path change during clean confirmation, and managed operation failure.
- [ ] Confirm the new tests fail before moving service logic.
- [ ] Move workspace and editor business state into the session; keep egui tab focus and cursor state in the UI.
- [ ] Ensure every file read/write is confined to the selected canonical workspace and writes retain atomic replacement/conflict detection.
- [ ] Rerun the Studio suite and commit workspace/editor migration.

### Task 8: Move authoring, composition editing, and content creation

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_service/src/session/authoring.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/session/creation.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/source_authoring.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/content_creation.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/workspace_content.rs

**Interfaces:**
- Authoring actions load/save composition dependencies, Workshop metadata, runtime outputs, and validate source content.
- Save requests carry the expected source fingerprint/content so stale edits remain conflict-safe.
- Creation actions carry Rust-defined content kind, identity, dependency choices, and optional Workshop policy; the service validates through Vapor before writing.

**Steps:**
- [ ] Add tests for invalid identity, missing required dependency slots, duplicate dependency choice, stale manifest edits, invalid runtime outputs, and recoverable failed creation.
- [ ] Confirm tests fail before service actions are implemented.
- [ ] Move validation and state transitions into the service while calling existing Vapor authoring and workspace functions for writes.
- [ ] Keep only unsaved text-field buffers and presentation validation hints in egui.
- [ ] Rerun the Studio suite and commit authoring and creation migration.

### Task 9: Move Workshop discovery, acquisition, subscriptions, and publishing

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_service/src/session/workshop.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/session/publishing.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/workshop.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/root_publishing.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workshop_search_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workshop_subscriptions_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workshop_acquisition_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/publish.rs

**Interfaces:**
- Workshop actions cover search, subscription snapshots/changes, single/batch acquisition, staging, publication, and root distribution review/publication.
- Acquisitions return durable app-cache identities, never temporary Steam download paths.
- Publish actions require the same explicit reviewed confirmation data as the current UI; no scenario can instantiate the production publisher.

**Steps:**
- [ ] Add fixture-backed scenarios for query errors, subscription refresh, duplicate IDs, partial batch download, unavailable item, reviewed-stage mismatch, explicit publish confirmation, and root upload failure.
- [ ] Confirm tests fail before moving Workshop workflow state.
- [ ] Move Workshop state and post-acquisition refresh decisions into the service; reuse Vapor's Steamworks and publication authorities.
- [ ] Ensure service traces never include credentials, temporary download locations, or unredacted environment values.
- [ ] Rerun the Studio suite and commit Workshop/share migration.

### Task 10: Centralize operation execution, progress, cancellation, and history

**Files:**
- Modify: Vapor-Studio/crates/vapor_studio_service/src/session.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/operations.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/operations.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/operation_protocol.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/operation_results.rs

**Interfaces:**
- The service owns the single active-operation gate, operation identity, queued/running/final states, progress, bounded history, and refresh-after-completion rules.
- StudioEvent::OperationChanged includes operation ID, domain/action, phase, progress, status, and state revision.
- Cancellation is exposed only for operations whose provider can cooperatively stop; unsupported cancellation returns a typed result.

**Steps:**
- [ ] Add tests for monotonic progress events, terminal events exactly once, stale worker results, disconnected worker failure, bounded history, and unsupported cancellation.
- [ ] Confirm tests fail before the service executor is introduced.
- [ ] Move OperationService::run_observed use into the service executor and adapt Vapor operation events into protocol events.
- [ ] Remove the UI-owned mpsc worker dispatcher and result-driven refresh rules after all action groups use the service.
- [ ] Rerun the Studio suite and commit operation lifecycle migration.

### Task 11: Implement stdio server and sibling-process packaging

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_service/src/server.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/main.rs
- Modify: Vapor-Studio/crates/vapor_studio_service/Cargo.toml
- Modify: Vapor-Studio/Workspace.vapor.toml

**Interfaces:**
- run_stdio_service(context_args) -> Result<(), ServiceExitError> reads one versioned JSON frame per line and writes one response/event frame per line.
- The sibling service executable accepts a private stdio mode and does not initialize egui or print logs to protocol stdout.
- Vapor-Studio package declares vapor-studio-service as a managed binary; Vapor promotes it beside vapor-gui in the shared app-root binary directory.

**Steps:**
- [ ] Add server tests for handshake, request/response correlation, malformed frames, unsupported versions, bounded shutdown, and stderr-only diagnostics.
- [ ] Confirm tests fail before the service binary exists.
- [ ] Implement a single input loop plus serialized writer; route long work through the service executor so incoming shutdown and cancellation messages remain responsive.
- [ ] Add the service Cargo binary target and declare it in Vapor-Studio/Workspace.vapor.toml.
- [ ] Verify vapor-studio-service and vapor-gui are both discovered and promoted beside each other by Vapor's managed app build.
- [ ] Run vapor test vapor-studio and vapor check vapor-gui; commit server and packaging changes.

### Task 12: Implement the typed client and child lifecycle supervisor

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_client/Cargo.toml
- Create: Vapor-Studio/crates/vapor_studio_client/src/lib.rs
- Create: Vapor-Studio/crates/vapor_studio_client/src/process.rs
- Create: Vapor-Studio/crates/vapor_studio_client/src/transport.rs
- Modify: Vapor-Studio/crates/vapor_studio_service/tests/process.rs

**Interfaces:**
- StudioClient::spawn(service_path, bootstrap) -> Result<StudioClient, ClientError>
- StudioClient::send(action, expected_revision) -> Result<RequestId, ClientError>
- StudioClient::try_recv() -> Result<Option<StudioEvent>, ClientError>
- StudioClient::shutdown(self, timeout) -> Result<(), ClientError>
- The process supervisor caps frame size, checks protocol version, reaps children, and bounds all waits; it resolves vapor-studio-service beside the deployed vapor-gui executable.

**Steps:**
- [ ] Add process integration tests under the service package, where Cargo exposes its own binary path, for handshake, snapshots, action round-trip, wrong version, early exit, malformed output, and shutdown timeout.
- [ ] Confirm tests fail before the client exists.
- [ ] Implement stdin writer and stdout reader as separate owned tasks with a bounded event channel; send service stderr to a separate diagnostic reader.
- [ ] Locate the sibling service executable from the deployed GUI binary directory, with an explicit override only for development.
- [ ] Test Linux and Windows executable naming/path resolution through injected path cases.
- [ ] Run vapor test vapor-studio and vapor test vapor-gui; commit the client.

### Task 13: Add the developer terminal simulator and scenario runner

**Files:**
- Create: Vapor-Studio/crates/vapor_studio_service/src/simulator.rs
- Create: Vapor-Studio/crates/vapor_studio_service/src/scenario.rs
- Create: Vapor-Studio/crates/vapor_studio_service/scenarios/fixture-profile-selection.json
- Create: Vapor-Studio/crates/vapor_studio_service/scenarios/workspace-open-recovery.json
- Create: Vapor-Studio/crates/vapor_studio_service/scenarios/fixture-publisher-boundary.json
- Modify: Vapor-Studio/crates/vapor_studio_service/src/main.rs
- Modify: Vapor-Studio/README.md

**Interfaces:**
- vapor-studio-service simulate --scenario FILE starts a service child with fixtures, sends the scenario's typed actions through the same client protocol, and checks declared state/result expectations.
- Interactive simulation accepts a small line-oriented command set that maps one-to-one to StudioAction; help, snapshot, trace, and quit are always available.
- Scenario output includes action, correlation ID, previous/current revision, operation events, and outcome.

**Steps:**
- [ ] Add scenario parser tests for valid sequences, unknown actions, malformed expectations, and prohibited production-provider selection.
- [ ] Confirm those tests fail before simulator commands exist.
- [ ] Implement scenario playback using the real child-process protocol and fixture-only providers.
- [ ] Implement an interactive terminal client over StudioClient; do not reimplement validation or workflow transitions in the terminal adapter.
- [ ] Run all three checked-in fixture scenarios through the managed service binary and assert their terminal outcomes. Keep scenario names scoped to implemented fixture behavior until the full lifecycle actions are migrated.
- [ ] Commit simulator and scenario fixtures.

### Task 14: Migrate GUI startup, Home, profiles, and Play to the service

**Files:**
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/app.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/app_shell.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/home.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/library.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/library_profile_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/library_composition_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/library_installation_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/library_launch_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/profiles.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/operation_center.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/Cargo.toml

**Interfaces:**
- StudioApp owns StudioClient, latest StudioSnapshot, and presentation-only view state.
- UI callbacks submit semantic actions and render server snapshots/events; they do not invoke vapor_studio_core operations.
- Startup transfers legacy egui storage only as a one-time migration seed.

**Steps:**
- [ ] Add UI adapter tests proving profile selection, install, and launch controls send exact semantic actions and render returned action availability.
- [ ] Confirm adapter tests fail before the client is wired.
- [ ] Spawn and handshake with the service before first discovery; show a recoverable service error view if startup fails.
- [ ] Migrate Home, profile management, composition install/verify/repair, and Play to service snapshots/actions.
- [ ] Verify a failed service launch remains visible as a failed operation and never shows a false success.
- [ ] Run vapor test vapor-studio and vapor check vapor-gui; commit this vertical slice.

### Task 15: Migrate Discover, Create, composition editing, and Workshop UI

**Files:**
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/catalog.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/content_creation.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/composition_lifecycle.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/discover.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/create.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/composition.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workshop_browser.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workshop_search_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workshop_subscriptions_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workshop_acquisition_panel.rs

**Interfaces:**
- Every catalog, creation, composition, and Workshop control maps to a StudioAction and reads the current service snapshot.
- Selection, stale snapshot warnings, batch selection, import/subscribe distinctions, and partial acquisition outcomes come from service state.

**Steps:**
- [ ] Add adapter tests for all controls in these screens, including disabled actions and preserved retry selections.
- [ ] Confirm the adapter tests fail before the screen migrations.
- [ ] Migrate the four screen groups to client requests and service projections without changing their visual design.
- [ ] Remove direct core operation calls from these modules.
- [ ] Run Studio and GUI managed suites; commit Discover/Create/Workshop migration.

### Task 16: Migrate Workspace, Editor, Publish, and remaining UI actions

**Files:**
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/source_authoring.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/workspace_content.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/workspace_editor.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/workspace_files.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/workspace_ops.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workspace.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workspace_authoring_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workspace_content_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workspace_files_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workspace_operations_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/workspace_registry_panel.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/pages/publish.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/root_publishing.rs
- Modify: Vapor-Studio/crates/vapor_studio_ui/src/operation_results.rs

**Interfaces:**
- Workspace, authoring, editor, build, Workshop publishing, and root publishing controls use the same typed action/event interface.
- UI-only text drafts remain local until submitted; service owns validated saved state and conflict/error results.

**Steps:**
- [ ] Add adapter tests for guarded file save, external workspace change, clean confirmation, authoring save, Workshop review, and publish confirmation actions.
- [ ] Confirm adapter tests fail before migration.
- [ ] Migrate remaining UI areas and remove the old OperationResult, OperationState::spawn, and result-driven workflow reducer from the UI.
- [ ] Search the UI crate for direct calls to mutating vapor_studio_core functions; retain only pure display formatting and type access.
- [ ] Run Studio and GUI managed suites; commit the full GUI client migration.

### Task 17: Close protocol, scenario, packaging, and cross-platform gaps

**Files:**
- Modify: Vapor-Studio/crates/vapor_studio_protocol/src/
- Modify: Vapor-Studio/crates/vapor_studio_service/src/
- Modify: Vapor-Studio/crates/vapor_studio_client/src/
- Modify: Vapor-Studio/crates/vapor_studio_service/scenarios/
- Modify: Vapor-Studio/Workspace.vapor.toml
- Modify: Vapor-Studio/README.md
- Modify: .agents/workstreams/open/workshop-content-superpass.md

**Interfaces:**
- All state snapshots carry monotonic revisions; all operations produce one terminal event and structured failure.
- All scenarios run through the process protocol and fixture providers.
- The service binary is included in the same platform runtime package as vapor-gui.

**Steps:**
- [ ] Add the complete default-profile launch scenario from clean install state through dependency resolution, acquisition fixture, install, verification, and launch handoff.
- [ ] Add scenario checks for service restart, state persistence migration, failed provider recovery, and exact error visibility.
- [ ] Verify every protocol action is exercised by either a service unit test, a process integration scenario, or a GUI adapter test.
- [ ] Run vapor validate vapor-studio and vapor validate vapor-gui from the Vapor-Client source context; inspect full results and fix failures.
- [ ] Build managed Linux and Windows targets and confirm both packaged binaries resolve beside each other without Steam running.
- [ ] Update operator documentation with simulator invocation, scenario format, logs, and known visual-only review responsibilities.
- [ ] Update the active workstream with test/build evidence and remaining visual/manual gaps; commit and push final service integration.
- [ ] Stop before local deploy or Steam publication; those remain separate owner-controlled steps.

---

## Completion checklist

- [ ] The GUI has no workflow orchestration path outside StudioClient.
- [ ] The terminal simulator and GUI use identical protocol versions, actions, snapshots, and operation events.
- [ ] Deterministic scenarios cover every action family and failure/recovery path without external account mutation.
- [ ] Existing profiles, workspace selection, editor drafts, and operation history survive the one-time storage migration and service restarts.
- [ ] Service crashes, incompatible versions, malformed frames, and shutdown are bounded and user-visible.
- [ ] Linux and Windows managed builds include the service beside vapor-gui.
- [ ] A human visual review remains required for layout, hierarchy, legibility, and interaction feel.
