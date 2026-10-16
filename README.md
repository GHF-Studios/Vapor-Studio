# Vapor Studio

Vapor Studio is the creator-facing product UI for the local library,
compositions, workspace authoring, publishing review, diagnostics, and play
selection. Its `vapor_studio_ui` crate is hosted by Vapor-GUI and calls shared
Vapor operations for domain behavior.

The **Compose** screen edits the source dependency graph of a local
Packagepack. It loads a snapshot through Vapor Shell, lets the creator add or
remove workspace content edges, then asks Shell to enforce role/cardinality,
version, cycle, and local-descendant rules before replacing the manifest. A
saved graph can become the active profile root and be resolved through the
normal composition preview. Cache and installed Packagepacks remain read-only.

The Workspace screen also surfaces projects from the explicit Vapor project
registry. It shows their role, version, dependencies, and declared runtime
binaries/libraries, provides per-project validation, and links to each manifest
and to Packagepack graph editing/profile selection. The
Packagepack composer also exposes version requirements, optional edges, and
dependency reasons; Vapor Shell validates them before saving.

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

The Workspace project overview also opens a structured Workshop metadata
editor for existing source projects. Saving checks that the opened manifest
snapshot is still current, preserves unrelated manifest fields, validates the
updated role manifest through Vapor Shell discovery, and atomically replaces
the file.
Packagepack graph and publication-metadata edits share a per-manifest lock.
