# Vapor Studio

Vapor Studio is the creator-facing product UI for the local library,
compositions, workspace authoring, publishing review, diagnostics, and play
selection. Its `vapor_studio_ui` crate is hosted by Vapor-GUI and calls shared
Vapor operations for domain behavior.

The **Compose** screen edits source dependency graphs for locally authored
content projects. It loads a snapshot through Vapor Shell, lets the creator add
or remove workspace content edges, then asks Shell to enforce role,
cardinality, version, cycle, and local-descendant rules before replacing the
manifest. A saved Packagepack graph can become the active profile root and be
previewed directly against workspace, cache, and installed content, with source
provenance, blockers, and Workshop acquisition links shown beside the graph.
That preview is read-only; cache and installed content remain unchanged.
Any saved source project can also move straight from Compose to a staged
Workshop review. Studio opens the existing review surface; publishing still
requires the separate explicit review and confirmation.

The Workspace screen separates workspace operations and content projects into
an Overview, while source editing lives in a Files view. The Overview surfaces
projects from the explicit Vapor project registry. It shows their role,
version, dependencies, and declared runtime
binaries/libraries, provides per-project validation, and links to each manifest
and dependency editor. Packagepack dependency editing can also select that
Packagepack as the active profile root. The composer exposes version
requirements, optional edges, and dependency reasons; Vapor Shell validates
them before saving.

Studio's Workspace screen can find valid, unregistered content-project folders
and show scan diagnostics. The creator explicitly chooses which candidate to
adopt. Vapor validates its role manifest and identity, then registers the folder
without moving it or changing Cargo membership.

The Create screen can author optional Steam Workshop publication metadata
alongside a new content project. These fields are written through Vapor Shell
into that content role's manifest; creating a project does not stage or publish
it. The Workspace project overview reads the same Shell-validated metadata back
so creators can inspect the intended AppID, item identity, visibility, title,
and tags before staging a review package.

The Workspace file editor opens a bounded set of text sources with TOML syntax
feedback and contextual guidance. Saves use a per-file lock, check the original
snapshot again before replacement, preserve file permissions, and restore the
original if replacement fails. Creating a file can make missing parent folders
inside the workspace, while rejecting path escapes and symlinked folders.

The Workspace project overview also opens a structured Workshop metadata
editor for existing source projects. Saving checks that the opened manifest
snapshot is still current, preserves unrelated manifest fields, validates the
updated role manifest through Vapor Shell discovery, and atomically replaces
the file.
Packagepack graph, publication-metadata, and runtime-output edits share one
per-manifest lock and atomic replacement path.

The same project overview opens a structured runtime-output editor for declared
executables and libraries. Names are validated by Vapor Shell, and updates use
the same stale-snapshot check, shared manifest lock, unknown-field preservation,
atomic replacement, and post-save discovery as Workshop metadata. Changing
outputs clears each affected profile's installation plan so it must be resolved
and installed again before Play. Any profile pinned to the exact edited content
version is invalidated; profiles pinned to parallel versions remain intact.

Each project can also be unregistered from Vapor discovery through a
confirmation step. This removes only its `[[workspace.projects]]` entry; the
project files and Cargo workspace membership remain unchanged. The existing
Find projects / Adopt flow can register it again later.

## Developer service simulator

The Studio workspace declares `vapor-studio-service` as a managed binary for
promotion beside `vapor-gui` by a shared Vapor application-root build. The GUI
and terminal simulator use the same versioned protocol and service state. To
inspect workflow behavior without opening the GUI, run the promoted service:

```text
vapor-studio-service simulate
vapor-studio-service simulate --scenario crates/vapor_studio_service/scenarios/fixture-profile-selection.json
vapor-studio-service simulate --scenario crates/vapor_studio_service/scenarios/fixture-workspace-discovery.json
```

The simulator starts a separate fixture-only service process. Its provider set
contains no Steam or Workshop publisher, and the checked-in scenarios exercise
only deterministic local state and rejection paths. The interactive commands
include `open WORKSPACE_PATH`, `discover`, `refresh`, `search`, and `inspect`;
workspace actions return the same typed source-project and adoption-candidate
snapshot consumed by the GUI.
