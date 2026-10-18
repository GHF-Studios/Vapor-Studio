# Vapor Studio content lifecycle implementation plan

> Required implementation workflow: `superpowers:executing-plans`.

**Goal:** Deliver a coherent player and creator lifecycle through one shared
Vapor Client service, with the same meanings and results in Studio, CLI,
Installer, and Steam launch paths.

**Architecture:** Vapor defines content semantics once. The Vapor Client
service owns application workflows, durable state, and provider effects. Each
frontend is an adapter over the same Rust API and local process. Composition
is the user and domain concept; existing profile records are only its current
storage representation. The ownership contract and dependency assessment are
in `Vapor/docs/vapor-client-service.md`.

## Invariants

- Existing composition/profile data and installed content remain readable.
- Subscribe makes content available in Library; it never silently edits a
  composition. Adding an add-on requires a compatible explicit target.
- Library Auto-Install is a user-owned lifecycle policy: enabled items follow
  the latest version compatible with every dependent composition and are
  automatically removed from the local Library/cache once no composition needs
  them; retained items stay available until the user explicitly updates or
  removes them. This policy is not authored in a content manifest and is
  separate from resolving required dependencies for a single install operation.
- Vapor's Rust-defined content/pack vocabulary is authoritative.
- One canonical Superworkspace defaults to `~/Vapor`; changing it is explicit.
- The active authoring workspace is independent of every composition. Folder
  changes and composition selection/creation do not mutate another aggregate;
  source-workspace bindings are explicit composition data.
- Player flows say Find, Subscribe, Install, Add, and Play. Cache paths,
  receipts, fingerprints, and storage records stay in support diagnostics.
- All launch paths use the same service bootstrap and connection manager.
  Steam Entrypoint invokes that manager before handing off; it has no duplicate
  service or workflow implementation.
- Use Vapor-managed build/deploy operations at the Vapor-Client root.

## Ordered work

### 1. Define the application domain before relocating executables

**Authority:** `Vapor/crates/vapor_content`, `vapor_client_domain`,
`vapor_client_protocol`, and the Vapor Client application API. `vapor_core`
is an adapter/host layer, not an alternate semantic owner.

- [x] Map every remaining Studio/core/CLI operation to one canonical domain
  concept or presentation-only role. Composition records, transitions,
  catalog reconciliation, install planning, and local content/provider
  operations now have shared Vapor owners. Service persistence, request
  orchestration, and operation coordination remain in the Studio host adapter.
- [x] Remove `vapor_cli::manifest::ContentKind` as a second enum. The shell
  now uses the core kind, manifest filenames derive from the core type, and
  the CLI now uses `Crate` for Rust-only static code content.
- [x] Route shell dependency-kind checks through Vapor's shared rule instead
  of keeping a second pack/member table.
- [x] Put optional-edge default behavior in the shared dependency model:
  optional content remains authored and visible to install planning but is not
  auto-selected into the required resolved graph.
- [x] Replace protocol `ContentIdentity` with shared `ContentReference` and
  validate its IDs/exact versions through `VaporId` and `ContentVersionId`.
  `ResolvedCompositionGraph` is now explicitly a wire/storage projection of
  `vapor_content::ResolvedComposition` plus installation observations, with
  checked conversions and no independent resolver policy.
- [x] Make the install-plan node, dependency, and provider-outcome records one
  shared application type across Shell, Studio core, and the protocol. The
  protocol preview wrapper keeps its composition identity, while payload nodes
  use the domain contract directly.
- [x] Make composition semantics the API and durable-state model; preserve
  schema 1 profile persistence through an explicit one-time schema 2 migration.
- [x] Keep content identity, pack rules, dependencies, compatibility, and
  resolution in shared Vapor semantics. Identity, pack rules, dependency
  declarations, optional-edge behavior, graph/pack compatibility validation,
  composition validation, and generic recursive resolution now live in
  `vapor_content`; the shared traversal accepts provider-selected exact
  candidates.
- [x] Replace Shell's duplicate dependency traversal with the shared resolver
  and keep source availability/provenance in its application projection.
  Provider selection remains an adapter closure; graph traversal, exact-edge
  checks, cycle detection, pack compatibility, and dependency-first ordering
  are shared.
- [x] Adapt `vapor_core::LocalCatalog` and the Workspace content catalog to
  shared typed `ContentCandidates`; the exact recursive graph resolver and
  resolved node/composition types now live in `vapor_content`. The generic
  `Vapor.toml` schema remains an input adapter, not a second graph model.
- [x] Move role-specific authored manifest parsing, typed identity/version
  validation, and implied composition edges into `vapor_content::authored_manifest`.
  Shell source discovery, source editing, and deployed cache/installation
  ingestion now consume that shared contract.
- [x] Replace Shell's separate dependency traversal with one provider-neutral
  resolver plus a separate install-availability/provenance plan. The planner
  reports workspace, cache, installed, and Workshop-seed outcomes.
- [x] Define durable Library ownership policy (`AutoInstall` versus retained)
  in the Vapor-owned application domain, including latest-compatible version
  selection, retained-version behavior, and reachability-based collection rules;
  persist the preference and expose it through the shared service protocol. The
  composition and Library action types are now domain-owned intents.
  Constraint collection now spans every resolved composition and fails closed
  on legacy or malformed edges missing a persisted range. Automatic update and
  collection orchestration still need to consume that policy.
- [x] Extract canonical content identities, kinds, pack/dependency rules, and
  authored role schema into `vapor_content`, a host-independent crate with no
  CLI dependency. `vapor_core` retains compatibility re-exports while its
  CLI and host orchestration remain outside that crate.
- [x] Establish `vapor_client_domain` as the host-independent owner for
  application policy above content semantics; its first rule defines Library
  Auto-Install versus retained-version and collection behavior. Durable service
  state and operations still need to consume that rule.
- [x] Put shared content workflows and provider operations behind Vapor-owned
  contracts. Composition/Library intents, catalog reconciliation, install
  planning, source-workspace operations, and filesystem/Workshop content
  operations now have one implementation in `vapor_client_domain` and
  `vapor_client_provider`; Studio Core consumes that provider API as a host
  adapter.
- [x] Remove Studio-named public protocol/client crates and public aliases;
  the active shared API crates are `vapor_client_protocol` and
  `vapor_client`. Studio-internal service/UI types remain appropriately
  frontend-named.
- [x] Resolve the dependency direction `Studio service -> Studio core -> Vapor
  CLI`. Studio Core now depends on Vapor's provider crate; Vapor CLI re-exports
  that same implementation as a frontend facade. The shared provider crate has
  no dependency on either frontend.

**Exit condition:** one reviewed domain graph, with every current operation
assigned one owner and no unresolved dependency inversion hidden by a crate
rename.

### 2. Build the real shared service host and lifecycle

**Owner:** Vapor Client shared code and a Vapor-owned service-host crate.

- [x] Separate process-level `ClientHostContext` from per-frontend
  `ClientHostConfig`. The host/application crates now live in Vapor.
- [x] Implement platform-safe local IPC and one active service for the selected
  Vapor installation; each facade connects to it instead of starting a private
  workflow child. The client uses `interprocess` local sockets, a locked
  singleton startup path, per-user endpoint permissions, and idle shutdown
  after frontends leave.
- [x] Give the common bootstrap manager responsibility for discovery,
  startup, version negotiation, stale endpoint recovery, and shutdown policy.
- [x] Keep legacy GUI-storage import service-owned and one-time. Entrypoint
  readiness checks don't create durable state; the first real frontend supplies
  an optional seed and the service imports it only if no durable state exists.
- [x] Have Steam Entrypoint call the common bootstrap manager and then hand off
  to the requested frontend. GUI and `vapor client launch` use the same manager
  through the entrypoint.
- [x] Support independent clients, fresh snapshots on connect, correlated
  requests, operation subscriptions, reconnect, and truthful recovery errors.

**Exit condition:** multiple simultaneous frontends observe one persisted
service state and one operation lifecycle; restarting one frontend does not
restart or reset the service.

### 3. Establish shared workspace and source context

- [x] Persist the canonical Superworkspace separately from the active
  authoring folder and a composition's optional source-workspace binding.
  Default to `~/Vapor`; setup and changing its location are explicit service
  operations. First real service connection initializes the default; explicit
  location changes never rewrite the active authoring folder.
- [x] Model first-run, initialized, unavailable, and changed-location states
  for the canonical `~/Vapor` Superworkspace.
- [x] Discover and initialize it through service-owned actions; setup is
  idempotent and location changes are explicit.
- [x] Acquire source repositories through service-owned Vapor operations;
  registry discovery is explicit and asynchronous, selected repositories are
  acquired through Vapor's validated ecosystem operation, and Studio offers
  acquired workspaces for authoring without asking users to paste paths.
- [x] Return recoverable state for invalid locations and missing sources.

### 4. Complete Library, Workshop, and composition lifecycle

- [x] Represent item kind and next valid action in authoritative service state.
- [x] Keep discovery, subscription, acquisition, installation, Add, and Play
  as distinct actions with correlated outcomes.
- [x] Install a complete Packagepack as a launchable composition.
- [x] Keep local catalog actions available when Steam is unavailable, with
  provider readiness accurately represented.

### 5. Add content to an editable composition safely

- [x] Offer only compositions whose Packagepack source is editable in the
  current authoring workspace and whose dependency rules accept the item.
- [x] Route the selected target into its Packagepack dependency editor, add the
  item to an unsaved draft, and require an explicit save before changing source.
- [ ] Make a downloaded Workshop artifact editable through an explicit fork;
  never imply a cached runtime copy was added to a composition.
- [ ] Complete Auto-Install update and reachability-based collection
  orchestration before exposing it as a working user toggle.

### 6. Align creation and navigation to the same model

- [x] Present exact Rust-defined artifact names with concise descriptions.
- [x] Group creator destinations in a visibly distinct area without a global
  player/creator mode switch.
- [x] Keep optional choices inside the flow they modify.
- [ ] Make primary actions and current operation state stable across narrow and
  wide layouts.

### 7. Verify end-to-end behavior and distribution

- [x] Use Vapor-managed compilation and local deployment for affected projects.
- [ ] Exercise service scenarios for setup, subscription, target selection,
  install, launch readiness, concurrency, reconnect, and persistence when the
  user authorizes tests.
- [x] Deploy locally through Vapor and confirm compilation and deployment of
  the shared service and Studio facade.
- [ ] Record Linux/Windows runtime evidence separately; no Steam publication
  without an explicit request.

- [ ] Review nested repository diffs and ensure only the intended Vapor and
  Studio changes are included.
- [ ] Commit and push nested repositories before updating and pushing their
  parent submodule references.

## Current implementation evidence and gap

The client/protocol, application, local provider, and service-host crates are
Vapor-owned. Studio consumes Vapor-owned application contracts; the renamed
`vapor_cli` package re-exports shared provider operations as a frontend facade.
Pass 1's ownership review is complete. The shared client and service now use
one local IPC process per installation and state directory. Linux local
deployment compiled and deployed the changes. Runtime concurrency and Windows
behavior remain unverified under this pass's deployment-only verification
constraint.

Pass 3 keeps canonical Superworkspace configuration in Vapor installation
source state, separate from the durable active authoring folder and composition
source bindings. The first real service connection initializes `~/Vapor` when
needed; service actions expose explicit relocation, registry discovery, and
repository acquisition. Studio can select a repository from the registry,
acquire it through Vapor, and open acquired Vapor Workspaces without typing
their paths. Acquisition no longer changes the active authoring folder.

The repository audit found that `vapor_core::LocalCatalog` consumes the
provisional generic `Vapor.toml`, while the active content system uses
role-specific manifests. Both now adapt candidates to shared exact-graph
semantics; `vapor_client_domain` owns planning and `vapor_client_provider`
supplies source/cache/install observations. The broad `vapor_core` crate still
combines content compatibility exports with host, CLI, and Cargo operations;
that boundary needs further deliberate separation before it is a narrow core.

Passes 4–5 now expose service-projected item roles/actions, distinct Workshop
subscribe/download/update controls, explicit eligible-composition selection,
and a draft-only add path through the existing Packagepack source editor.
Pass 6 groups the Rust-defined creator artifact kinds and retains the separate
Creator navigation area. Local deployment compiled and deployed both the
service and Studio facade. A downloaded Workshop runtime item still cannot be
forked into an editable source project, and Auto-Install's durable preference
still lacks update/collection orchestration; neither is presented as working
functionality. Tests and runtime checks were not run per the user's constraints.
