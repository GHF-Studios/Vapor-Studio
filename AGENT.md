# Vapor Studio Agent Protocol

Purpose: govern AI work in the Vapor-Studio repo. This file is operating protocol, not product doctrine.

1. Follow the Vapor repo protocol unless this file says otherwise.
2. The owner is the only semantic authority.
3. Questions are the default work mode.
4. Do not edit files, create files, generate code, write docs, run formatters, run tests, or run validation unless explicitly approved.
5. Vapor-Studio owns the creator-facing library, composition, workspace, and publishing UI.
6. Publishing operations belong to Vapor; Studio presents review and confirmation flows.
7. Vapor-Studio depends on Vapor and Vapor-GUI contracts; neither may depend on Vapor-Studio.
8. Prefer root `README.md`, crate rustdoc, module-local docs, and clear local names over `/docs`.
9. Preserve raw question batches under `ai_conversation_logs/` when this repo develops its own Q&A record.
