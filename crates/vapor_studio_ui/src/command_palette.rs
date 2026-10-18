use super::catalog::DiscoverTab;
use super::workshop::WorkshopTab;
use super::{Section, StudioApp};
use eframe::egui;

#[derive(Clone, PartialEq, Eq)]
enum CommandPaletteAction {
    Navigate(Section),
    Workshop(WorkshopTab),
    ValidateWorkspace,
    WorkspaceOperation(vapor_core::DevelopmentOperation),
    OpenWorkspace,
    OpenWorkspaceFiles,
    RefreshLocalContent,
    LocalContent(String),
    WorkspaceCandidate(String),
    Composition(String),
    SearchWorkshop(String),
}

fn command_palette_entry_matches(title: &str, keywords: &str, query: &str) -> bool {
    let title = title.to_lowercase();
    query
        .split_whitespace()
        .all(|term| title.contains(term) || keywords.contains(term))
}

impl StudioApp {
    pub(super) fn open_command_palette(&mut self) {
        self.command_palette_open = true;
        self.command_palette_search.clear();
        self.command_palette_focus_search = true;
        self.command_palette_selected_index = 0;
    }

    pub(super) fn draw_command_palette(&mut self, ui: &mut egui::Ui) {
        if !self.command_palette_open {
            return;
        }

        let entries = [
            (
                "Play",
                "launch current composition",
                CommandPaletteAction::Navigate(Section::Home),
            ),
            (
                "Compositions",
                "play install update compositions",
                CommandPaletteAction::Navigate(Section::Compositions),
            ),
            (
                "Library",
                "downloaded installed content available add composition",
                CommandPaletteAction::Navigate(Section::Library),
            ),
            (
                "Browse content",
                "local content catalog workshop browse search",
                CommandPaletteAction::Navigate(Section::Discover),
            ),
            (
                "Browse Steam Workshop",
                "search acquire browse remote",
                CommandPaletteAction::Workshop(WorkshopTab::Browse),
            ),
            (
                "Workshop subscriptions",
                "subscribed download unsubscribe installed",
                CommandPaletteAction::Workshop(WorkshopTab::Subscriptions),
            ),
            (
                "Downloaded content",
                "downloaded workshop imports subscribe",
                CommandPaletteAction::Workshop(WorkshopTab::Imports),
            ),
            (
                "Check project folder",
                "check project setup and required content",
                CommandPaletteAction::ValidateWorkspace,
            ),
            (
                "Format workspace",
                "fmt rust source",
                CommandPaletteAction::WorkspaceOperation(vapor_core::DevelopmentOperation::Fmt),
            ),
            (
                "Check workspace",
                "compiler type check",
                CommandPaletteAction::WorkspaceOperation(vapor_core::DevelopmentOperation::Check),
            ),
            (
                "Test workspace",
                "tests verification",
                CommandPaletteAction::WorkspaceOperation(vapor_core::DevelopmentOperation::Test),
            ),
            (
                "Build workspace",
                "compile executable",
                CommandPaletteAction::WorkspaceOperation(vapor_core::DevelopmentOperation::Build),
            ),
            (
                "Clean workspace",
                "remove derived artifacts",
                CommandPaletteAction::WorkspaceOperation(vapor_core::DevelopmentOperation::Clean),
            ),
            (
                "Create content",
                "new project template author",
                CommandPaletteAction::Navigate(Section::Create),
            ),
            (
                "Projects",
                "source projects files build",
                CommandPaletteAction::Navigate(Section::Workspace),
            ),
            (
                "Composition",
                "packagepack dependencies composition",
                CommandPaletteAction::Navigate(Section::Compose),
            ),
            (
                "Share content",
                "steam workshop review upload",
                CommandPaletteAction::Navigate(Section::Publish),
            ),
            (
                "Diagnostics",
                "logs telemetry runtime",
                CommandPaletteAction::Navigate(Section::Diagnostics),
            ),
            (
                "Open project folder",
                "folder select source",
                CommandPaletteAction::OpenWorkspace,
            ),
            (
                "Open project files",
                "project editor source code",
                CommandPaletteAction::OpenWorkspaceFiles,
            ),
            (
                "Refresh local content",
                "rescan catalog workspace",
                CommandPaletteAction::RefreshLocalContent,
            ),
        ];
        let mut window_open = true;
        let mut requested_action = None;
        let mut close_requested = false;
        let mut request_focus = self.command_palette_focus_search;
        self.command_palette_focus_search = false;
        let viewport = ui.ctx().content_rect().size();
        let popup_size = egui::vec2((viewport.x - 24.0).max(1.0), (viewport.y - 24.0).max(1.0));
        let results_height = (viewport.y - 250.0).clamp(40.0, 340.0);

        egui::Window::new("Command palette")
            .id(egui::Id::new("studio-command-palette"))
            .collapsible(false)
            .resizable(false)
            .default_width((viewport.x - 24.0).clamp(1.0, 560.0))
            .max_size(popup_size)
            .anchor(
                egui::Align2::CENTER_TOP,
                egui::vec2(0.0, if viewport.y < 600.0 { 12.0 } else { 72.0 }),
            )
            .open(&mut window_open)
            .show(ui.ctx(), |ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.command_palette_search)
                        .desired_width(ui.available_width())
                        .hint_text("Search pages, actions, or content"),
                );
                if request_focus {
                    response.request_focus();
                    request_focus = false;
                }
                ui.weak("↑ / ↓ move · Enter open · Esc close");
                let query = self.command_palette_search.trim().to_lowercase();
                if response.changed() {
                    self.command_palette_selected_index = 0;
                }
                let enter_pressed = response.has_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::Enter));
                let move_down = response.has_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::ArrowDown));
                let move_up = response.has_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::ArrowUp));
                let escape_pressed = ui.input(|input| input.key_pressed(egui::Key::Escape));
                let can_switch_workspace = !self.operation.running
                    && !self.has_unsaved_context_switch_edits();
                let can_refresh = !self.operation.running
                    && !self.has_unsaved_workspace_edits();
                let can_run_workspace_action = can_refresh
                    && !self.workspace.root.trim().is_empty();
                let enabled = |action: &CommandPaletteAction| match action {
                    CommandPaletteAction::Navigate(_) | CommandPaletteAction::Workshop(_) => true,
                    CommandPaletteAction::ValidateWorkspace => can_run_workspace_action,
                    CommandPaletteAction::WorkspaceOperation(_) => {
                        can_run_workspace_action
                            && self.workspace.pending_clean_confirmation.is_none()
                    }
                    CommandPaletteAction::OpenWorkspace => can_switch_workspace,
                    CommandPaletteAction::OpenWorkspaceFiles => {
                        !self.operation.running && !self.workspace.root.trim().is_empty()
                    }
                    CommandPaletteAction::RefreshLocalContent => can_refresh,
                    CommandPaletteAction::LocalContent(_) => true,
                    CommandPaletteAction::WorkspaceCandidate(_) | CommandPaletteAction::Composition(_) => {
                        can_switch_workspace
                    }
                    CommandPaletteAction::SearchWorkshop(_) => !self.operation.running,
                };
                let mut palette_entries = entries
                    .iter()
                    .map(|(title, terms, action)| {
                        ((*title).to_owned(), (*terms).to_owned(), action.clone())
                    })
                    .collect::<Vec<_>>();
                if query.chars().count() >= 2 {
                    let active_composition_id = self
                        .composition
                        .items
                        .get(self.composition.active_index)
                        .map(|composition| composition.id.clone());
                    let mut local_matches = self
                        .catalog
                        .items
                        .iter()
                        .filter(|content| {
                            let searchable = format!(
                                "{} {} {} {} {}",
                                content.id,
                                content.kind,
                                content.version.as_deref().unwrap_or_default(),
                                content.published_file_id.as_deref().unwrap_or_default(),
                                if content.is_unambiguous() { "ready" } else { "conflict needs review" },
                            )
                            .to_lowercase();
                            query
                                .split_whitespace()
                                .all(|term| searchable.contains(term))
                        })
                        .take(8)
                        .map(|content| {
                            let identity = super::catalog_content_selector(content);
                            let state = if content.is_unambiguous() {
                                ""
                            } else {
                                " · needs review"
                            };
                            let title = format!("{} · {}{}", identity, content.kind, state);
                            (
                                title,
                                format!(
                                    "local content {} {} {} {} {}",
                                    content.id,
                                    content.kind,
                                    content.version.as_deref().unwrap_or_default(),
                                    content.published_file_id.as_deref().unwrap_or_default(),
                                    if content.is_unambiguous() { "ready" } else { "conflict needs review" },
                                ),
                                CommandPaletteAction::LocalContent(identity),
                            )
                        })
                        .collect::<Vec<_>>();
                    palette_entries.append(&mut local_matches);
                    palette_entries.extend(
                        self.composition
                            .items
                            .iter()
                            .filter(|composition| {
                                let active = active_composition_id.as_deref() == Some(composition.id.as_str());
                                let root = composition
                                    .root_package
                                    .as_ref()
                                    .map(super::content_selector)
                                    .unwrap_or_default();
                                let state = super::components::design::installation_status(
                                    &composition.installation,
                                )
                                .0;
                                let searchable = format!(
                                    "{} {root} {state} {}",
                                    composition.name,
                                    if active { "active current selected" } else { "" },
                                )
                                .to_lowercase();
                                query
                                    .split_whitespace()
                                    .all(|term| searchable.contains(term))
                            })
                            .take(8)
                            .map(|composition| {
                                let root = composition
                                    .root_package
                                    .as_ref()
                                    .map(super::content_selector)
                                    .unwrap_or_else(|| "no composition".to_owned());
                                let state = super::components::design::installation_status(
                                    &composition.installation,
                                )
                                .0;
                                let active = active_composition_id.as_deref()
                                    == Some(composition.id.as_str());
                                (
                                    format!(
                                        "Composition · {} · {}{}",
                                        composition.name,
                                        state,
                                        if active { " · active" } else { "" },
                                    ),
                                    format!(
                                        "launch composition {root} {state} {}",
                                        if active { "active current selected" } else { "" },
                                    )
                                    .to_lowercase(),
                                    CommandPaletteAction::Composition(composition.id.clone()),
                                )
                            }),
                    );
                    palette_entries.extend(
                        self.workspace
                            .unregistered_projects
                            .candidates
                            .iter()
                            .filter(|candidate| {
                                let searchable = format!(
                                    "{} {} {}",
                                    candidate.id,
                                    candidate.kind,
                                    candidate.path.display(),
                                )
                                .to_lowercase();
                                query
                                    .split_whitespace()
                                    .all(|term| searchable.contains(term))
                            })
                            .take(8)
                            .map(|candidate| {
                                (
                                    format!("{} · {} · add to Studio", candidate.id, super::display_content_kind(&candidate.kind.to_string())),
                                    format!(
                                        "project folder content {} {} {} add",
                                        candidate.id,
                                        super::display_content_kind(&candidate.kind.to_string()),
                                        candidate.path.display(),
                                    ),
                                    CommandPaletteAction::WorkspaceCandidate(candidate.id.clone()),
                                )
                            }),
                    );
                    palette_entries.push((
                        format!("Search Steam Workshop for ‘{}’", self.command_palette_search.trim()),
                        "remote workshop browse search online".to_owned(),
                        CommandPaletteAction::SearchWorkshop(
                            self.command_palette_search.trim().to_owned(),
                        ),
                    ));
                }
                let matching_entries = palette_entries
                    .iter()
                    .filter(|(title, terms, _)| {
                        command_palette_entry_matches(title, terms, &query)
                    })
                    .collect::<Vec<_>>();
                let enabled_entries = matching_entries
                    .iter()
                    .filter(|(_, _, action)| enabled(action))
                    .collect::<Vec<_>>();
                if !enabled_entries.is_empty() {
                    let last_index = enabled_entries.len() - 1;
                    if move_down {
                        self.command_palette_selected_index =
                            (self.command_palette_selected_index + 1) % enabled_entries.len();
                    } else if move_up {
                        self.command_palette_selected_index = if self.command_palette_selected_index == 0 {
                            last_index
                        } else {
                            self.command_palette_selected_index - 1
                        };
                    } else {
                        self.command_palette_selected_index =
                            self.command_palette_selected_index.min(last_index);
                    }
                } else {
                    self.command_palette_selected_index = 0;
                }
                let selected_action = enabled_entries
                    .get(self.command_palette_selected_index)
                    .map(|(_, _, action)| (*action).clone());
                let mut match_count = 0;
                egui::ScrollArea::vertical()
                    .max_height(results_height)
                    .show(ui, |ui| {
                        for (title, _terms, action) in matching_entries {
                            match_count += 1;
                            let is_selected =
                                enabled(action) && selected_action.as_ref() == Some(action);
                            let response = ui.add_enabled(
                                enabled(action),
                                egui::Button::selectable(is_selected, title.as_str())
                                    .min_size(egui::vec2(ui.available_width(), 32.0)),
                            );
                            if response.clicked() {
                                requested_action = Some(action.clone());
                            }
                            if !enabled(action) {
                                response.on_hover_text("Finish the current operation or save/discard edits first.");
                            }
                        }
                    });
                if match_count == 0 {
                    ui.weak("No matching pages or content. Search local content or Steam Workshop with a longer query.");
                }
                if enter_pressed {
                    requested_action = selected_action;
                }
                if escape_pressed {
                    close_requested = true;
                }
            });

        window_open &= !close_requested && requested_action.is_none();
        self.command_palette_open = window_open;
        if !window_open || requested_action.is_some() {
            self.command_palette_search.clear();
            self.command_palette_focus_search = false;
        }
        match requested_action {
            Some(CommandPaletteAction::Navigate(section)) => {
                self.activate_navigation_section(section);
            }
            Some(CommandPaletteAction::Workshop(tab)) => {
                self.open_workshop_tab(tab);
            }
            Some(CommandPaletteAction::ValidateWorkspace) => self.validate_workspace_source(),
            Some(CommandPaletteAction::WorkspaceOperation(operation)) => {
                self.run_operation(operation);
            }
            Some(CommandPaletteAction::OpenWorkspace) => self.open_workspace_picker(),
            Some(CommandPaletteAction::OpenWorkspaceFiles) => {
                self.open_workspace_files();
            }
            Some(CommandPaletteAction::RefreshLocalContent) => {
                self.open_discover_tab(DiscoverTab::Local);
                self.discover();
            }
            Some(CommandPaletteAction::LocalContent(selector)) => {
                self.catalog.search_query = self
                    .catalog
                    .items
                    .iter()
                    .find(|content| super::catalog_content_selector_matches(content, &selector))
                    .map(|content| content.id.clone())
                    .unwrap_or_else(|| selector.clone());
                self.select_catalog_identity(selector);
                self.catalog.source_filter = crate::navigation::CatalogSourceFilter::All;
                self.catalog.kind_filter = None;
                self.open_discover_tab(DiscoverTab::Local);
            }
            Some(CommandPaletteAction::WorkspaceCandidate(id)) => {
                self.workspace.content_search = id;
                self.open_workspace_panel(super::workspace_ops::WorkspacePanel::Content);
            }
            Some(CommandPaletteAction::Composition(composition_id)) => {
                if let Some(index) = self
                    .composition
                    .items
                    .iter()
                    .position(|composition| composition.id == composition_id)
                {
                    self.select_composition(index);
                    self.activate_navigation_section(Section::Compositions);
                }
            }
            Some(CommandPaletteAction::SearchWorkshop(query)) => {
                self.workshop.search_query = query;
                self.open_workshop_tab(WorkshopTab::Browse);
                self.search_workshop();
            }
            None => {}
        }
    }
}
