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
