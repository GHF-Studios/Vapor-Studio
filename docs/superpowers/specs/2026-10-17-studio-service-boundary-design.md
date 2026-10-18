# Historical: Vapor Studio Service Boundary

> This short-lived stdio-child design is superseded by the shared
> [Vapor Client Service architecture](../../../Vapor/docs/vapor-client-service.md).
> Keep this document as implementation history; do not extend its one-client,
> GUI-owned process model.

## Purpose

Separate Studio behavior from its visual presentation so the same user workflows can be driven through the GUI, a terminal simulator, and automated scenarios. The service must make incorrect workflow state and operation results observable without requiring screenshots or a human to inspect every interaction.

This design preserves the existing product direction: Studio is the full-stack launcher and authoring environment; Vapor owns content resolution, installation, managed tools, and publishing authority.

## Current evidence

The vapor_studio_core crate already wraps many real content, profile, workspace, and Workshop operations. The vapor_studio_ui crate currently starts those operations in its own worker-thread dispatcher, interprets results, refreshes data, and owns much of the workflow state. Consequently, calling core functions directly does not exercise the same state transitions and result handling used by the GUI.

## Architecture

Ship a normal, short-lived vapor-studio-service child application with Studio. It is not an OS service or persistent daemon. The GUI starts one service process for its session, connects over private standard input/output, and stops it when the session closes. A terminal simulator starts the same service and uses the same protocol. There is no network listener.

The service is the application boundary. It owns:

- Studio session and profile/workspace selection that changes available operations.
- Workflow state machines, action availability, validation, and operation orchestration.
- Operation identity, progress, cancellation where supported, completion, failure, and refresh decisions.
- User-meaningful state snapshots and structured diagnostics.

The GUI owns rendering, focus, scroll, window layout, and other presentation-only state. It sends semantic actions (for example, InstallSelectedComposition) rather than asking the service to click or draw controls. It renders the service's current snapshot and events. The terminal client issues the same actions and displays equivalent summaries. Neither client reimplements workflow rules.

The service bootstraps once from its arguments and inherited environment, resolves an explicit host context (application root, workspace, platform, and available providers), and passes that context to operations. Business operations do not read ambient process environment directly. Secrets are not included in snapshots, protocol logs, or error messages.

## Protocol and lifecycle

Use a versioned, typed JSON-RPC-style protocol over the child process's standard streams. Requests carry semantic actions and correlation IDs; responses return typed results or structured errors; notifications carry state revisions and operation progress. Standard output is protocol-only and diagnostics go to standard error. A newly connected client can request a complete snapshot and then apply newer revisions.

The parent owns process lifetime and detects early exit, malformed messages, and protocol-version mismatch. Shutdown is explicit and bounded; the parent terminates an unresponsive child. The initial implementation supports one interactive client per service process. Service state is not silently persisted across process restarts; durable profiles, workspace settings, and operation history continue to use their existing storage contracts.

## Providers and side effects

The service reuses Vapor's existing authority and provider implementations. It does not duplicate Steam, Workshop, toolchain, content, or publishing logic. A narrow provider boundary permits deterministic in-memory or temporary-directory implementations for scenarios, while production uses the real Vapor providers.

Scenarios must be able to exercise discovery, profile selection, composition resolution, acquisition, installation, verification, launch handoff, authoring, and recoverable failures. External or destructive effects such as Workshop publication must be intercepted by a fixture in automated scenarios and remain subject to the existing explicit confirmation flow in production.

## Functional and presentation verification

Treat the action catalog, state transitions, action availability, operation events, and user-visible errors as service contracts. Cover each meaningful action and failure/recovery path through deterministic scenarios; prioritize workflow coverage over a line-coverage target. Scenario traces must identify the action, prior and resulting state revisions, operation events, and final outcome.

This makes functional failures observable without rendering. It does not claim to judge typography, layout, visual hierarchy, readability, or subjective usability. Those remain a separate human visual review against the same service snapshots. A GUI issue is presentation-related only after the terminal or protocol trace confirms that the service received the intended action and produced the expected state.

## Migration and scope

First inventory every GUI-triggered workflow and define its semantic action, state transition, result, and external effects. Then introduce the service/session API and protocol, add the terminal simulator and deterministic providers, and migrate the GUI to become a client. Keep existing content types and Vapor authority boundaries unchanged. Do not combine this work with a visual redesign.

The terminal simulator is a developer-facing client for the same service, not a second implementation of Studio and not a replacement for the GUI. The service executable and protocol remain version-locked with the GUI shipped in the same Vapor composition.

## Acceptance

- GUI and terminal clients can perform the same supported semantic actions through the same service.
- A scripted scenario can reproduce a complete profile launch workflow and report the exact state transition or operation that failed.
- Deterministic scenarios cover successful paths, unavailable prerequisites, provider failures, cancellation where supported, and recovery.
- Automated scenarios cannot publish or mutate a real Steam account.
- The GUI contains no duplicate workflow orchestration or direct calls to Studio content operations after migration.
- A human can assess presentation separately while relying on service traces to establish functional state.

## Implementation status

The initial implementation slice provides the typed protocol, bounded child
process client, stdio server, fixture simulator, service-owned local catalog
refresh/search/selection, workspace-root activation, and active-profile ID
selection. Other action families are declared in the protocol but still return
a structured unavailable-action error. Profiles and operation history remain
in their existing GUI storage, several workspace scans and all long-running
operations still use the GUI worker dispatcher, and provider fixture ports,
service persistence, and complete GUI migration remain future slices in the
implementation plan. `vapor validate --project vapor-studio` passes, and the
managed service binary runs checked-in scenarios through a child process. A
host application-root build was attempted, but currently stops at `vapor-gui`:
its ignored local `Cargo.lock` needs regeneration while the managed build
enforces `--locked`; the service's promotion beside `vapor-gui` is therefore
not yet verified.
