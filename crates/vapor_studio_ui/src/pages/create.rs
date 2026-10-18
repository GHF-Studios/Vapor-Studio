use super::super::StudioApp;
use super::super::components::design::{
    ACCENT, StatusTone, confirmation_card, discard_confirmation, page_header, status_text,
    use_stacked_form,
};
use eframe::egui;
use std::path::Path;

impl StudioApp {
    pub(in crate::app) fn draw_create(&mut self, ui: &mut egui::Ui) {
        let operation_running = self.operation.running;
        ui.add_enabled_ui(!operation_running, |ui| self.draw_create_content(ui));
    }

    fn draw_create_content(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            "Creator",
            "Create a project",
            "Choose what you are making. Studio creates the starter files, then guides you through its name and required content.",
        );
        if self.authoring.create.is_dirty()
            && !self.authoring.create.discard_confirmation_open
            && ui.button("Discard current content draft").clicked()
        {
            self.authoring.create.discard_confirmation_open = true;
        }
        if self.authoring.create.discard_confirmation_open
            && let Some(discard) = discard_confirmation(
                ui,
                "Discard this new-content draft?",
                "This clears the project identity and selected content in this draft. No project files have been created yet.",
            )
        {
            if discard {
                self.authoring.create.reset();
                self.status = "Discarded the new content project draft.".to_owned();
            } else {
                self.authoring.create.discard_confirmation_open = false;
            }
        }
        self.draw_create_flow(ui);
    }

    fn draw_create_flow(&mut self, ui: &mut egui::Ui) {
        self.draw_starter_workspace_section(ui);

        if self.workspace.root.trim().is_empty() {
            ui.group(|ui| {
                ui.heading("Choose where to create your project");
                ui.label("Start from a template above, or open a folder that already contains your projects.");
                if ui.button("Open an existing project folder").clicked() {
                    self.open_workspace_picker();
                }
            });
            return;
        }

        ui.horizontal_wrapped(|ui| {
            ui.weak(format!("Project folder · {}", self.workspace.root));
            if ui.small_button("Change").clicked() {
                self.activate_navigation_section(super::super::Section::Workspace);
            }
        });

        if !self.authoring.create.details_open {
            self.draw_content_template_step(ui);
            return;
        }

        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.authoring.create.pending_dependency_clear
                        && self.authoring.create.pending_role_replacement.is_none(),
                    egui::Button::new("← Templates"),
                )
                .clicked()
            {
                self.authoring.create.details_open = false;
            }
            ui.label(
                egui::RichText::new("STEP 2 OF 2 · DETAILS")
                    .small()
                    .strong()
                    .color(ACCENT),
            );
            ui.heading(format!(
                "New {} project",
                super::super::content_kind_name(self.authoring.create.content_kind)
            ));
        });
        ui.label("Give this project a name and choose the content it needs. Steam Workshop details are optional and can be added later.");
        ui.separator();
        let role_constraints = match self.authoring.create.content_kind {
            vapor_core::ContentKind::Packagepack => {
                "Choose one game engine and one game for this setup."
            }
            vapor_core::ContentKind::Game => "Choose one game engine for this game to run on.",
            vapor_core::ContentKind::Enginepack => {
                "Choose one game engine to include in this setup."
            }
            vapor_core::ContentKind::Gamepack => "Choose one game to include in this setup.",
            vapor_core::ContentKind::Crate => "Choose any Rust Crates this crate depends on.",
            _ => "Required content is checked against the type of project you are creating.",
        };
        let source_shape = match self.authoring.create.content_kind {
            vapor_core::ContentKind::Packagepack
            | vapor_core::ContentKind::Enginepack
            | vapor_core::ContentKind::Gamepack
            | vapor_core::ContentKind::Modpack => {
                "This creates a reusable setup without creating a code project."
            }
            vapor_core::ContentKind::Engine => {
                "This creates a code project that provides the program used to run a game."
            }
            vapor_core::ContentKind::Game
            | vapor_core::ContentKind::EngineMod
            | vapor_core::ContentKind::GameMod
            | vapor_core::ContentKind::ExtensionMod => {
                "This creates a code project that adds game behavior or extensions."
            }
            vapor_core::ContentKind::Crate => {
                "This creates a plain Rust crate built for static linking. It has no runtime-loaded game behavior."
            }
        };
        let required_role_groups = vapor_client_application::source_required_dependency_choice_groups(
            self.authoring.create.content_kind,
        );
        let selected_in_role_group = |required_group: &&str| {
            self.catalog.sources.iter().any(|content| {
                self.authoring
                    .create
                    .dependencies
                    .iter()
                    .any(|selected| selected == &super::super::source_content_selector(content))
                    && super::super::source_content_kind(&content.kind).is_some_and(|kind| {
                        vapor_client_application::source_dependency_choice_group(
                            self.authoring.create.content_kind,
                            kind,
                        ) == Some(*required_group)
                    })
            })
        };
        let required_role_selected = required_role_groups.iter().all(selected_in_role_group);
        if let Some(error) = self.catalog.source_discovery_error.as_deref() {
            status_text(
                ui,
                &format!(
                    "Workspace content refresh failed. The last successful list is shown, but creating with these dependencies is disabled until refresh succeeds: {error}"
                ),
                StatusTone::Caution,
            );
        }
        let unavailable_selected_dependencies = self
            .authoring
            .create
            .dependencies
            .iter()
            .filter(|selected| {
                !self.catalog.sources.iter().any(|content| {
                    super::super::source_content_selector(content) == selected.as_str()
                })
            })
            .count();
        ui.small(source_shape);
        if use_stacked_form(ui) {
            ui.label("Content ID");
            ui.add(
                egui::TextEdit::singleline(&mut self.authoring.create.content_id)
                    .desired_width(ui.available_width()),
            );
            ui.label("Version");
            ui.add(
                egui::TextEdit::singleline(&mut self.authoring.create.content_version)
                    .desired_width(ui.available_width()),
            );
        } else {
            egui::Grid::new("new-content-identity")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Content ID");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.authoring.create.content_id)
                            .desired_width(320.0),
                    );
                    ui.end_row();
                    ui.label("Version");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.authoring.create.content_version)
                            .desired_width(180.0),
                    );
                    ui.end_row();
                });
        }
        self.draw_workshop_metadata_section(ui);
        self.draw_create_dependency_section(
            ui,
            role_constraints,
            unavailable_selected_dependencies,
            required_role_selected,
        );
    }

    fn draw_create_dependency_section(
        &mut self,
        ui: &mut egui::Ui,
        role_constraints: &str,
        unavailable_selected_dependencies: usize,
        required_role_selected: bool,
    ) {
        let source_catalog_current = self.catalog.source_snapshot_current();
        let dependency_confirmation_open = self.authoring.create.pending_dependency_clear
            || self.authoring.create.pending_role_replacement.is_some();
        egui::CollapsingHeader::new("Content this project needs")
            .default_open(true)
            .open(dependency_confirmation_open.then_some(true))
            .show(ui, |ui| {
                ui.weak(role_constraints);
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Selected content · {}",
                        self.authoring.create.dependencies.len()
                    ));
                    if ui
                        .add_enabled(
                            !self.authoring.create.dependencies.is_empty()
                                && !self.authoring.create.pending_dependency_clear
                                && self.authoring.create.pending_role_replacement.is_none(),
                            egui::Button::new("Clear all"),
                        )
                        .clicked()
                    {
                        self.authoring.create.pending_dependency_clear = true;
                    }
                });
                if self.authoring.create.pending_dependency_clear {
                    let description = format!(
                        "This removes all {} selected content items from this project draft. The project identity and Workshop details stay in the draft.",
                        self.authoring.create.dependencies.len(),
                    );
                    if let Some(clear) = confirmation_card(
                        ui,
                        "Clear all selected content?",
                        &description,
                        "Clear selected content",
                        "Keep selection",
                        !self.operation.running,
                    ) {
                        if clear {
                            self.authoring.create.clear_dependencies();
                            self.status = "Cleared the new content project's selected content.".to_owned();
                        }
                        self.authoring.create.pending_dependency_clear = false;
                    }
                }
                if self.catalog.sources.is_empty() {
        ui.weak("Refresh the project list to choose included content.");
                } else {
                    ui.horizontal_wrapped(|ui| {
                        let search_width =
                            super::super::components::design::search_field_width(ui);
                        ui.add(
                            egui::TextEdit::singleline(
                                &mut self.authoring.create.dependency_search,
                            )
                            .desired_width(search_width)
                            .hint_text("Find compatible projects"),
                        );
                        if !self.authoring.create.dependency_search.is_empty()
                            && ui.small_button("Clear").clicked()
                        {
                            self.authoring.create.dependency_search.clear();
                        }
                    });
                    let selected_dependencies = self
                        .authoring
                        .create
                        .dependencies
                        .iter()
                        .map(|selected| {
                            let content = self.catalog.sources.iter().find(|content| {
                                super::super::source_content_selector(content) == *selected
                            });
                            let role = self
                                .authoring
                                .create
                                .exclusive_dependency_groups
                                .get(selected)
                                .copied();
                            (
                                selected.clone(),
                                content.map(|content| {
                                    format!(
                                        "{} · {}",
                                        super::super::display_content_kind(&content.kind),
                                        content.version.as_deref().unwrap_or("unversioned")
                                    )
                                }),
                                role,
                                self.authoring
                                    .create
                                    .optional_dependencies
                                    .contains(selected),
                            )
                        })
                        .collect::<Vec<_>>();
                    if !selected_dependencies.is_empty() {
                        ui.label("Selected for this draft");
                        ui.horizontal_wrapped(|ui| {
                            for (selected, detail, role, mut optional) in selected_dependencies {
                                let role_label = role.map(|role| format!("{role} · ")).unwrap_or_default();
                                let has_detail = detail.is_some();
                                let label = match detail {
                                    Some(detail) => format!("{role_label}{selected} · {detail}"),
                                    None => format!("{role_label}{selected} · unavailable after refresh"),
                                };
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(label);
                                    if role.is_none() && has_detail {
                                        if ui
                                            .add_enabled(
                                                !self.authoring.create.pending_dependency_clear
                                                    && self.authoring.create.pending_role_replacement.is_none(),
                                                egui::Checkbox::new(&mut optional, "Optional"),
                                            )
                                            .changed()
                                        {
                                            if optional {
                                                self.authoring
                                                    .create
                                                    .optional_dependencies
                                                    .insert(selected.clone());
                                            } else {
                                                self.authoring
                                                    .create
                                                    .optional_dependencies
                                                    .remove(&selected);
                                            }
                                        }
                                    }
                                    if ui
                                        .add_enabled(
                                            !self.authoring.create.pending_dependency_clear
                                                && self.authoring.create.pending_role_replacement.is_none(),
                                            egui::Button::new("Remove"),
                                        )
                                        .clicked()
                                    {
                                        self.authoring.create.remove_dependency(&selected);
                                    }
                                });
                            }
                        });
                    }
                    if unavailable_selected_dependencies > 0 {
                        status_text(
                            ui,
                            &format!(
                                "{unavailable_selected_dependencies} selected content item{} {} no longer available. Remove them or refresh the project list before continuing.",
                                if unavailable_selected_dependencies == 1 { "" } else { "s" },
                                if unavailable_selected_dependencies == 1 { "is" } else { "are" }
                            ),
                            StatusTone::Caution,
                        );
                    }
                    let query = self.authoring.create.dependency_search.trim().to_lowercase();
                    let mut visible_dependencies = 0;
                    egui::ScrollArea::vertical()
                .max_height(ui.available_height().clamp(0.0, 240.0))
                        .show(ui, |ui| {
                            for content in &self.catalog.sources {
                                let child_kind =
                                    super::super::source_content_kind(&content.kind);
                                let allowed = child_kind.is_some_and(|kind| {
                                    vapor_client_application::source_dependency_allowed(
                                        self.authoring.create.content_kind,
                                        kind,
                                    )
                                });
                                if !allowed {
                                    continue;
                                }
                                let id = super::super::source_content_selector(content);
                                let matches_query = query.is_empty()
                                    || id.to_lowercase().contains(&query)
                                    || content.kind.to_lowercase().contains(&query)
                                    || content
                                        .version
                                        .as_deref()
                                        .unwrap_or_default()
                                        .to_lowercase()
                                        .contains(&query);
                                if !matches_query {
                                    continue;
                                }
                                visible_dependencies += 1;
                                let role_group = child_kind.and_then(|kind| {
                                    vapor_client_application::source_dependency_choice_group(
                                        self.authoring.create.content_kind,
                                        kind,
                                    )
                                });
                                let mut included = self
                                    .authoring
                                    .create
                                    .dependencies
                                    .iter()
                                    .any(|value| value == &id);
                                let choice_help = if role_group.is_some() {
                                    "This is the project's required game engine or game. Selecting it replaces the current choice for that role."
                                } else {
                                    "Select any number of supported dependencies."
                                };
                                if ui
                                    .add_enabled(
                                        !self.authoring.create.pending_dependency_clear
                                            && self.authoring.create.pending_role_replacement.is_none(),
                                        egui::Checkbox::new(
                                            &mut included,
                                            format!(
                                                "{} · {} · {}",
                                                id,
                                                super::super::display_content_kind(&content.kind),
                                                content.version.as_deref().unwrap_or("unversioned")
                                            ),
                                        ),
                                    )
                                    .on_hover_text(choice_help)
                                    .changed()
                                {
                                    if included {
                                        self.authoring
                                            .create
                                            .request_dependency_selection(id, role_group);
                                    } else {
                                        self.authoring.create.remove_dependency(&id);
                                    }
                                }
                            }
                        });
                    if visible_dependencies == 0 {
                        if query.is_empty() {
                            ui.weak("No compatible content is available yet. Create a project from a template or add an existing project folder.");
                        } else {
                            ui.weak("No compatible projects match this search.");
                        }
                    }
                }
            });
        if let Some(replacement) = self.authoring.create.pending_role_replacement.clone() {
            let target_is_available = self.catalog.sources.iter().any(|content| {
                super::super::source_content_selector(content) == replacement.target
            });
            let description = if target_is_available {
                format!(
                    "Replace {} with {} in the required content role? Other selected content stays in this draft.",
                    replacement.replacing.join(", "),
                    replacement.target,
                )
            } else {
                format!(
                    "{} is no longer available in the project list. Cancel this replacement, refresh content, then choose it again.",
                    replacement.target,
                )
            };
            if let Some(replace) = confirmation_card(
                ui,
                "Replace the current role choice?",
                &description,
                "Replace role choice",
                "Keep current choice",
                !self.operation.running && target_is_available,
            ) {
                self.authoring.create.resolve_role_replacement(replace);
            }
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.operation.running
                        && !self.has_unsaved_workspace_edits()
                        && !self.authoring.create.pending_dependency_clear
                        && self.authoring.create.pending_role_replacement.is_none(),
                    egui::Button::new("Refresh available content"),
                )
                .on_disabled_hover_text("Save or discard your open edits first; Studio reads saved project definitions.")
                .clicked()
            {
                self.discover();
            }
            if ui
                .add_enabled(
                    !self.operation.running
                        && source_catalog_current
                        && !self.has_unsaved_workspace_edits()
                        && !self.authoring.create.pending_dependency_clear
                        && self.authoring.create.pending_role_replacement.is_none()
                        && unavailable_selected_dependencies == 0
                        && required_role_selected,
                    egui::Button::new("Create content project"),
                )
                .on_disabled_hover_text(if !source_catalog_current {
                    "Refresh the project list before creating this project."
                } else if !required_role_selected {
                    "Choose the required game engine or game before creating this project."
                } else if unavailable_selected_dependencies > 0 {
                    "Remove selected projects that are no longer available, or refresh the project list."
                } else {
                    "Save or discard your open edits first; Studio opens the new project details when creation finishes."
                })
                .clicked()
            {
                self.create_content_workspace();
            }
        });
        ui.weak("Studio adds the new project to this folder after creating its files.");
    }

    fn draw_starter_workspace_section(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Start from a sample workspace · optional")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Creates example Engine, Game, and Packagepack projects together. Use this when you want a complete starting workspace.");
                ui.horizontal_wrapped(|ui| {
                    ui.label("Location");
                    let path_width = ui.available_width().min(360.0);
                    ui.add(egui::TextEdit::singleline(&mut self.workspace.starter_path)
                        .desired_width(path_width)
                        .hint_text("Where to create the project folder"));
                    if ui.button("Browse…").clicked() {
                        self.open_workspace_parent_picker();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Creator name");
                    ui.text_edit_singleline(&mut self.workspace.starter_organization);
                    ui.label("Project name");
                    ui.text_edit_singleline(&mut self.workspace.starter_name);
                });
                let organization_valid =
                    super::super::content_creation::starter_workspace_slug_is_valid(
                        self.workspace.starter_organization.trim(),
                    );
                let name_valid = super::super::content_creation::starter_workspace_slug_is_valid(
                    self.workspace.starter_name.trim(),
                );
                if !self.workspace.starter_organization.trim().is_empty() && !organization_valid {
                    status_text(
                        ui,
                        "Use lowercase letters, numbers, and hyphens for the creator name.",
                        StatusTone::Caution,
                    );
                }
                if !self.workspace.starter_name.trim().is_empty() && !name_valid {
                    status_text(
                        ui,
                        "Use lowercase letters, numbers, and hyphens for the project name.",
                        StatusTone::Caution,
                    );
                }
                ui.weak(
                    "Use lowercase letters, numbers, and hyphens. These names become part of the project's identity.",
                );
                if !self.workspace.starter_path.trim().is_empty()
                    && name_valid
                {
                    ui.small(format!(
                        "New project folder · {}",
                        Path::new(self.workspace.starter_path.trim())
                            .join(self.workspace.starter_name.trim())
                            .display()
                    ));
                }
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_context_switch_edits()
                            && !self.workspace.starter_path.trim().is_empty()
                            && organization_valid
                            && name_valid,
                        egui::Button::new("Create sample workspace"),
                    )
                    .on_disabled_hover_text(if !organization_valid || !name_valid {
                        "Enter a valid creator and project name to continue."
                    } else if self.operation.running {
                        "Wait for the current operation to finish before creating these projects."
                    } else if self.has_unsaved_context_switch_edits() {
                        "Discard the current draft or save/discard edits before opening another project folder."
                    } else if self.workspace.starter_path.trim().is_empty() {
                        "Choose where to create the sample project folder."
                    } else {
                        "Choose a folder you can write to."
                    })
                    .clicked()
                {
                    self.create_starter_workspace();
                }
                ui.weak("Studio creates the sample projects inside this folder. Advanced package settings use this Vapor installation's Steam app.");
            });
    }

    fn draw_content_template_step(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
                ui.label(egui::RichText::new("STEP 1 OF 2 · TEMPLATE").small().strong().color(ACCENT));
                ui.heading("Choose a Vapor content type");
                ui.label("These are the content types supported by Vapor. Each one has a different role and dependency rules.");
                let template_groups: [(&str, &[(vapor_core::ContentKind, &str, &str)]); 4] = [
                    (
                        "Playable setup",
                        &[(vapor_core::ContentKind::Packagepack, "Packagepack", "A complete playable composition with one Engine and one Game.")],
                    ),
                    (
                        "Reusable packs",
                        &[
                            (vapor_core::ContentKind::Enginepack, "Enginepack", "A reusable collection that selects an Engine."),
                            (vapor_core::ContentKind::Gamepack, "Gamepack", "A reusable collection that selects a Game."),
                            (vapor_core::ContentKind::Modpack, "Modpack", "A reusable collection of compatible add-ons."),
                        ],
                    ),
                    (
                        "Runtime and extensions",
                        &[
                            (vapor_core::ContentKind::Engine, "Engine", "The runtime that loads and runs a Game."),
                            (vapor_core::ContentKind::Game, "Game", "Game rules, assets, and behavior for an Engine."),
                            (vapor_core::ContentKind::EngineMod, "Engine Mod", "Adds behavior or tools to an Engine."),
                            (vapor_core::ContentKind::GameMod, "Game Mod", "Adds behavior or content to a Game."),
                            (vapor_core::ContentKind::ExtensionMod, "Extension Mod", "Adds reusable capabilities to supported content."),
                        ],
                    ),
                    (
                        "Rust code",
                        &[(vapor_core::ContentKind::Crate, "Crate", "Rust-only code for static linking into Engine, Game, and add-on projects.")],
                    ),
                ];
                let card_style = ui.style().clone();
                let template_change_pending =
                    self.authoring.create.pending_template_change.is_some();
                for (group_name, templates) in template_groups {
                    ui.add_space(8.0);
                    ui.strong(group_name);
                    ui.horizontal_wrapped(|ui| {
                    for &(kind, name, description) in templates {
                        egui::Frame::group(&card_style).show(ui, |ui| {
                            let card_width = ui.available_width().clamp(180.0, 260.0);
                            ui.set_min_width(card_width);
                            ui.set_max_width(card_width);
                            let selected = self.authoring.create.content_kind == kind;
                            if ui.add_enabled(
                                !template_change_pending,
                                egui::Button::selectable(selected, egui::RichText::new(name).strong())
                                    .min_size(egui::vec2(ui.available_width(), 34.0)),
                            ).clicked() && !selected {
                                if self.authoring.create.dependencies.is_empty() {
                                    self.authoring.create.content_kind = kind;
                                    self.authoring.create.dependency_search.clear();
                                } else {
                                    self.authoring.create.pending_template_change = Some(kind);
                                }
                            }
                            ui.small(description);
                        });
                    }
                });
                }
                if let Some(next_kind) = self.authoring.create.pending_template_change {
                    let description = format!(
                        "Changing from {} to {} clears the {} selected content items. The project identity and Workshop details stay in this draft.",
                        self.authoring.create.content_kind,
                        next_kind,
                        self.authoring.create.dependencies.len(),
                    );
                    if let Some(change) = confirmation_card(
                        ui,
                        "Change the template and clear its selected content?",
                        &description,
                        "Change template",
                        "Keep current template",
                        !self.operation.running,
                    ) {
                        if change {
                            self.authoring.create.content_kind = next_kind;
                            self.authoring.create.clear_dependencies();
                            self.authoring.create.dependency_search.clear();
                            self.status = format!(
                                "Changed the project type to {}; content choices were cleared.",
                                super::super::content_kind_name(next_kind)
                            );
                        }
                        self.authoring.create.pending_template_change = None;
                    }
                }
                let description = match self.authoring.create.content_kind {
                    vapor_core::ContentKind::Packagepack => "A playable setup that selects a game engine and a game.",
                    vapor_core::ContentKind::Enginepack => "A reusable setup that selects a game engine.",
                    vapor_core::ContentKind::Gamepack => "A reusable setup that selects a game.",
                    vapor_core::ContentKind::Modpack => "A reusable group of compatible add-ons.",
                    vapor_core::ContentKind::Engine => "A code project that provides a game engine.",
                    vapor_core::ContentKind::Game => "A code project that provides a game for an engine.",
                    vapor_core::ContentKind::EngineMod => "An add-on project for a game engine.",
                    vapor_core::ContentKind::GameMod => "An add-on project for a game.",
                    vapor_core::ContentKind::ExtensionMod => "A general add-on project.",
                    vapor_core::ContentKind::Crate => "A Rust-only crate for static linking, with no runtime-loaded content.",
                };
                ui.label(description);
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(
                        !self.operation.running
                            && self.authoring.create.pending_template_change.is_none(),
                        egui::Button::new("Continue"),
                    ).clicked() {
                        self.authoring.create.details_open = true;
                    }
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Refresh available projects")).clicked() {
                        self.discover();
                    }
                });
            });
    }

    fn draw_workshop_metadata_section(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Workshop publication metadata · optional")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("These optional settings prepare a future Workshop upload. Creating a project does not publish it.");
                if use_stacked_form(ui) {
                    ui.label("Steam AppID");
                    ui.add(egui::TextEdit::singleline(&mut self.authoring.create.workshop_app_id)
                        .desired_width(ui.available_width()));
                    ui.label("PublishedFileId");
                    ui.add(egui::TextEdit::singleline(&mut self.authoring.create.workshop_published_file_id)
                        .desired_width(ui.available_width()));
                } else {
                    egui::Grid::new("new-content-workshop-identity")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("Steam AppID");
                            ui.add(egui::TextEdit::singleline(&mut self.authoring.create.workshop_app_id).desired_width(180.0));
                            ui.end_row();
                            ui.label("PublishedFileId");
                            ui.add(egui::TextEdit::singleline(&mut self.authoring.create.workshop_published_file_id).desired_width(320.0));
                            ui.end_row();
                        });
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label("Visibility");
                    egui::ComboBox::from_id_salt("new-content-workshop-visibility")
                        .selected_text(if self.authoring.create.workshop_visibility.is_empty() {
                            "Not set"
                        } else {
                            &self.authoring.create.workshop_visibility
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.authoring.create.workshop_visibility, String::new(), "Not set");
                            for visibility in ["private", "friends-only", "public"] {
                                ui.selectable_value(
                                    &mut self.authoring.create.workshop_visibility,
                                    visibility.to_owned(),
                                    visibility,
                                );
                            }
                        });
                    ui.label("Title");
                    ui.text_edit_singleline(&mut self.authoring.create.workshop_title);
                });
                ui.label("Description");
                ui.add(
                    egui::TextEdit::multiline(&mut self.authoring.create.workshop_description)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.label("Tags · comma separated");
                    ui.text_edit_singleline(&mut self.authoring.create.workshop_tags);
                    ui.label("Change note");
                    ui.text_edit_singleline(&mut self.authoring.create.workshop_change_note);
                });
            });
    }
}
