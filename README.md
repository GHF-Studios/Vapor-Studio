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
registry. It shows their role, version, and dependencies, and provides direct
links to each manifest and to Packagepack graph editing/profile selection. The
Packagepack composer also exposes version requirements, optional edges, and
dependency reasons; Vapor Shell validates them before saving.

Studio's Workspace screen can find valid, unregistered content-project folders
and show scan diagnostics. The creator explicitly chooses which candidate to
adopt. Vapor validates its role manifest and identity, then registers the folder
without moving it or changing Cargo membership.
