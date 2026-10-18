# Vapor Studio content lifecycle

## Product contract

Studio is a launcher and content manager for complete playable compositions and the content they use. A composition is a Packagepack-backed, launchable setup. Existing profile-shaped saved data is converted once into the canonical composition record; “Profile” is not a separate product concept.

Players can find and subscribe to Workshop items, then see acquired non-Packagepack runtime content in Library even before installing it. Packagepacks remain in the separate Compositions area; Rust-only Crates are creator artifacts and stay out of player Library flows. Installing a standalone add-on is a separate intentional action: Studio asks which compatible composition should receive it, and offers existing editable compositions plus fork/create paths. Installing a complete Packagepack makes that composition available to launch. Download, cache, receipts, fingerprints, and paths are implementation details; ordinary flows use Find, Subscribe, Install, Add, and Play.

Studio uses one canonical Superworkspace, initialized at `~/Vapor` on the first real Vapor Client connection and discoverable from the installation context. The owner may change its location explicitly. Source repositories are discovered there and acquired through Vapor's controlled source operations; users do not paste repository paths into ordinary flows. Multiple canonical Superworkspaces are invalid.

The active authoring workspace belongs to service-level developer context and is independent of the selected playable composition. Opening a creator folder, creating a composition, or selecting another composition must not change that composition's Packagepack, resolved graph, installation state, or launch settings. A composition may carry an explicit source-workspace binding for source-run operations; only an explicit composition action may change that binding. The active folder is serialized as `authoring_workspace_root`; the prior `workspace_root` storage key remains a read alias for existing state.

Navigation gives Compositions, Library, and Browse distinct destinations; successful Workshop acquisition lands in Library. Player destinations and creator destinations live in one application without a mode switch. Creator destinations are grouped and visually marked as an advanced authoring area. Create uses Vapor's exact artifact vocabulary (Packagepack, Enginepack, Gamepack, Modpack, Engine, Game, Engine Mod, Game Mod, Extension Mod, Crate), each with a short explanation. Crate is Rust-only static build code and is never runtime content or a Packagepack member. Optional choices modify the flow that owns them.

The shared Vapor Client service owns workflow state, operation authority, and valid actions. Studio renders its state and submits typed actions through the shared API; the same service API is used by CLI, Installer, and Steam launch paths. User-facing failures describe recovery; implementation diagnostics remain available in App Status. Every supported launch path uses the common service bootstrap and connection manager.

## Migration constraints

- Preserve existing profile documents and installed compositions through an explicit schema 1 to schema 2 migration. New protocol and durable state use composition names; the former names exist only at that migration edge.
- Subscribe/acquire adds non-Packagepack content to the acquired-content Library before installation; it does not silently change a composition. Packagepacks are shown as playable compositions, not as Library items. An explicit Install/Add action selects a compatible target composition.
- Keep Workshop and local catalog available when Steam is unavailable, with truthful availability state.
- Do not expose local cache paths, receipts, fingerprints, or raw storage records in player flows.
- Preserve creator operations, but show them in a visibly distinct Creator area.
- Use Vapor-managed build and deployment operations from the Vapor-Client superworkspace.
- Route the lifecycle through the one shared Vapor Client Service described in
  `Vapor/docs/vapor-client-service.md`; Studio is a facade, not the owner of a
  private service process or a separate content policy. The service design is
  the target architecture from the outset; code migration does not justify
  Studio-specific aliases or a second workflow authority.
