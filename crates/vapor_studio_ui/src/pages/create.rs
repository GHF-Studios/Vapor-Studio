use super::super::StudioApp;
use super::super::components::design::{
    ACCENT, confirmation_card, discard_confirmation, page_header,
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
            "Author",
            "Create",
            "Start a content project from a template, then add its identity and required relationships.",
        );
        if self.authoring.create.is_dirty()
            && !self.authoring.create.discard_confirmation_open
            && ui.button("Discard current content draft").clicked()
        {
            self.authoring.create.discard_confirmation_open = true;
        }
        if self.authoring.create.discard_confirmation_open {
            if let Some(discard) = discard_confirmation(
                ui,
                "Discard this new-content draft?",
                "This clears the identity and dependency choices held in Studio. No project files have been created yet.",
            ) {
                if discard {
                    self.authoring.create.reset();
                    self.status = "Discarded the new content project draft.".to_owned();
                } else {
                    self.authoring.create.discard_confirmation_open = false;
                }
            }
        }
        egui::CollapsingHeader::new("Start from a complete playable workspace")
            .default_open(self.workspace.root.trim().is_empty())
            .show(ui, |ui| {
                ui.label("Scaffold an Engine, Game, Packagepack, Cargo projects, and workspace manifests.");
                ui.horizontal_wrapped(|ui| {
                    ui.label("Location");
                    ui.add(egui::TextEdit::singleline(&mut self.workspace.starter_path)
                        .desired_width(360.0)
                        .hint_text("Parent folder for the new workspace"));
                    if ui.button("Browse…").clicked() {
                        self.open_workspace_parent_picker();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Organization");
                    ui.text_edit_singleline(&mut self.workspace.starter_organization);
                    ui.label("Workspace name");
                    ui.text_edit_singleline(&mut self.workspace.starter_name);
                });
                if !self.workspace.starter_path.trim().is_empty()
                    && !self.workspace.starter_name.trim().is_empty()
                {
                    ui.small(format!(
                        "New workspace folder · {}",
                        Path::new(self.workspace.starter_path.trim())
                            .join(self.workspace.starter_name.trim())
                            .display()
                    ));
                }
                if ui.add_enabled(
                    !self.operation.running
                        && !self.has_unsaved_context_switch_edits()
                        && !self.workspace.starter_path.trim().is_empty(),
                    egui::Button::new("Create and open starter workspace"),
                )
                    .on_disabled_hover_text("Discard the Create draft or save/discard workspace edits before opening another workspace.")
                    .clicked()
                {
                    self.create_starter_workspace();
                }
                ui.weak("Studio creates a new folder named after the workspace inside this location. Its package metadata uses this Vapor installation's Steam AppID.");
            });

        if self.workspace.root.trim().is_empty() {
            ui.group(|ui| {
                ui.heading("Open a workspace to continue");
                ui.label("Content projects belong to an authored workspace. Create the starter workspace above or open one you already have.");
                if ui.button("Open existing workspace").clicked() {
                    self.open_workspace_picker();
                }
            });
            return;
        }

        ui.horizontal(|ui| {
            ui.weak(format!("Workspace · {}", self.workspace.root));
            if ui.small_button("Change").clicked() {
                self.activate_navigation_section(super::super::Section::Workspace);
            }
        });

        if !self.authoring.create.details_open {
            ui.group(|ui| {
                ui.label(egui::RichText::new("STEP 1 OF 2 · TEMPLATE").small().strong().color(ACCENT));
                ui.heading("Choose a content template");
                ui.label("Templates create one of Vapor's supported content roles. Packs describe compositions; code projects provide runtime behavior.");
                let templates = [
                    ("PACKS", vapor_core::ContentKind::Packagepack, "Packagepack", "A launchable composition with an Engine and optional Game."),
                    ("PACKS", vapor_core::ContentKind::Enginepack, "Enginepack", "A reusable Engine selection."),
                    ("PACKS", vapor_core::ContentKind::Gamepack, "Gamepack", "A reusable Game selection."),
                    ("PACKS", vapor_core::ContentKind::Modpack, "Modpack", "A reusable group of extensions."),
                    ("RUNTIME", vapor_core::ContentKind::Engine, "Engine", "A code project that provides an executable Engine."),
                    ("RUNTIME", vapor_core::ContentKind::Game, "Game", "A code project that provides gameplay for an Engine."),
                    ("EXTENSIONS", vapor_core::ContentKind::EngineMod, "Engine mod", "An extension for an Engine."),
                    ("EXTENSIONS", vapor_core::ContentKind::GameMod, "Game mod", "An extension for a Game."),
                    ("EXTENSIONS", vapor_core::ContentKind::ExtensionMod, "Extension", "A general-purpose content extension."),
                ];
                let card_style = ui.style().clone();
                let template_change_pending =
                    self.authoring.create.pending_template_change.is_some();
                for group in ["PACKS", "RUNTIME", "EXTENSIONS"] {
                    ui.small(group);
                    ui.horizontal_wrapped(|ui| {
                        for (_, kind, title, description) in templates.iter().filter(|(family, ..)| *family == group) {
                            egui::Frame::group(&card_style).show(ui, |ui| {
                                ui.set_min_width(175.0);
                                ui.set_max_width(220.0);
                                let selected = self.authoring.create.content_kind == *kind;
                                if ui.add_enabled(
                                    !template_change_pending,
                                    egui::Button::selectable(selected, egui::RichText::new(*title).strong())
                                        .min_size(egui::vec2(ui.available_width(), 34.0)),
                                ).clicked() && !selected {
                                    if self.authoring.create.dependencies.is_empty() {
                                        self.authoring.create.content_kind = *kind;
                                        self.authoring.create.dependency_search.clear();
                                    } else {
                                        self.authoring.create.pending_template_change = Some(*kind);
                                    }
                                }
                                ui.small(*description);
                            });
                        }
                    });
                }
                if let Some(next_kind) = self.authoring.create.pending_template_change {
                    let description = format!(
                        "Changing from {} to {} clears the {} selected dependencies. Your identity and Workshop metadata stay in the draft.",
                        self.authoring.create.content_kind,
                        next_kind,
                        self.authoring.create.dependencies.len(),
                    );
                    if let Some(change) = confirmation_card(
                        ui,
                        "Change the template and clear its dependency choices?",
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
                                "Changed the content template to {next_kind}; dependency choices were cleared."
                            );
                        }
                        self.authoring.create.pending_template_change = None;
                    }
                }
                let description = match self.authoring.create.content_kind {
                    vapor_core::ContentKind::Packagepack => "A launchable composition that selects an Engine and optional Game.",
                    vapor_core::ContentKind::Enginepack => "A reusable Engine composition with its required Engine implementation.",
                    vapor_core::ContentKind::Gamepack => "A reusable Game composition with its required Game implementation.",
                    vapor_core::ContentKind::Modpack => "A reusable group of compatible extensions.",
                    vapor_core::ContentKind::Engine => "A code project that supplies an executable Engine.",
                    vapor_core::ContentKind::Game => "A code project that supplies a Game for an Engine.",
                    vapor_core::ContentKind::EngineMod => "An Engine extension project.",
                    vapor_core::ContentKind::GameMod => "A Game extension project.",
                    vapor_core::ContentKind::ExtensionMod => "A general extension project.",
                    vapor_core::ContentKind::Library => "Choose a supported Vapor content role.",
                };
                ui.label(description);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(
                        !self.operation.running
                            && self.authoring.create.pending_template_change.is_none(),
                        egui::Button::new("Continue"),
                    ).clicked() {
                        self.authoring.create.details_open = true;
                    }
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Refresh workspace content")).clicked() {
                        self.discover();
                    }
                });
            });
            return;
        }

        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.authoring.create.pending_dependency_clear == false
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
                self.authoring.create.content_kind
            ));
        });
        ui.label("Set identity and required dependencies. Workshop details are optional and can be added later.");
        ui.separator();
        let role_constraints = match self.authoring.create.content_kind {
            vapor_core::ContentKind::Packagepack => {
                "Choose exactly one Engine or Enginepack; optionally choose at most one Game or Gamepack."
            }
            vapor_core::ContentKind::Game => "Choose exactly one Engine.",
            vapor_core::ContentKind::Enginepack => "Choose exactly one Engine.",
            vapor_core::ContentKind::Gamepack => "Choose exactly one Game.",
            _ => "Dependencies are checked against this role's supported content kinds.",
        };
        let source_shape = match self.authoring.create.content_kind {
            vapor_core::ContentKind::Packagepack
            | vapor_core::ContentKind::Enginepack
            | vapor_core::ContentKind::Gamepack
            | vapor_core::ContentKind::Modpack => {
                "Composition-only project: creates a role manifest without a Rust crate or Cargo member."
            }
            vapor_core::ContentKind::Engine => {
                "Code-bearing project: creates a Rust executable crate and declares its runtime binary."
            }
            vapor_core::ContentKind::Game
            | vapor_core::ContentKind::EngineMod
            | vapor_core::ContentKind::GameMod
            | vapor_core::ContentKind::ExtensionMod => {
                "Code-bearing project: creates a Rust library crate and declares its runtime library."
            }
            vapor_core::ContentKind::Library => {
                "Choose a supported Vapor content role to create a project."
            }
        };
        let required_role_group = vapor_studio_core::source_required_dependency_choice_group(
            self.authoring.create.content_kind,
        );
        let required_role_selected = required_role_group.is_none_or(|required_group| {
            self.catalog.sources.iter().any(|content| {
                self.authoring
                    .create
                    .dependencies
                    .iter()
                    .any(|selected| selected == &super::super::source_content_selector(content))
                    && super::super::source_content_kind(&content.kind).is_some_and(|kind| {
                        vapor_studio_core::source_dependency_choice_group(
                            self.authoring.create.content_kind,
                            kind,
                        ) == Some(required_group)
                    })
            })
        });
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
        if ui.available_width() < 640.0 {
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
        egui::CollapsingHeader::new("Workshop publication metadata · optional")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("These fields are authored into the role manifest. Creating content never publishes it.");
                if ui.available_width() < 640.0 {
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
        let dependency_confirmation_open = self.authoring.create.pending_dependency_clear
            || self.authoring.create.pending_role_replacement.is_some();
        egui::CollapsingHeader::new("Dependencies")
            .default_open(true)
            .open(dependency_confirmation_open.then_some(true))
            .show(ui, |ui| {
                ui.weak(role_constraints);
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Selected dependencies · {}",
                        self.authoring.create.dependencies.len()
                    ));
                    if ui
                        .add_enabled(
                            !self.authoring.create.dependencies.is_empty()
                                && !self.authoring.create.pending_dependency_clear
                                && self.authoring.create.pending_role_replacement.is_none(),
                            egui::Button::new("Clear selection"),
                        )
                        .clicked()
                    {
                        self.authoring.create.pending_dependency_clear = true;
                    }
                });
                if self.authoring.create.pending_dependency_clear {
                    let description = format!(
                        "This removes all {} dependency choices from the unsaved content draft. The project identity and Workshop metadata stay in the draft.",
                        self.authoring.create.dependencies.len(),
                    );
                    if let Some(clear) = confirmation_card(
                        ui,
                        "Clear every selected dependency?",
                        &description,
                        "Clear dependencies",
                        "Keep selection",
                        !self.operation.running,
                    ) {
                        if clear {
                            self.authoring.create.clear_dependencies();
                            self.status = "Cleared the new content project's dependency selections.".to_owned();
                        }
                        self.authoring.create.pending_dependency_clear = false;
                    }
                }
                if self.catalog.sources.is_empty() {
                    ui.weak("Refresh workspace content to choose dependencies.");
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(
                                &mut self.authoring.create.dependency_search,
                            )
                            .desired_width(300.0)
                            .hint_text("Filter compatible workspace content"),
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
                                        content.kind,
                                        content.version.as_deref().unwrap_or("unversioned")
                                    )
                                }),
                                role,
                            )
                        })
                        .collect::<Vec<_>>();
                    if !selected_dependencies.is_empty() {
                        ui.label("Selected for this draft");
                        ui.horizontal_wrapped(|ui| {
                            for (selected, detail, role) in selected_dependencies {
                                let role_label = role.map(|role| format!("{role} · ")).unwrap_or_default();
                                let label = match detail {
                                    Some(detail) => format!("{role_label}{selected} · {detail}"),
                                    None => format!("{role_label}{selected} · unavailable after refresh"),
                                };
                                ui.label(label);
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
                            }
                        });
                    }
                    if unavailable_selected_dependencies > 0 {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            format!(
                                "{unavailable_selected_dependencies} selected dependency(ies) are no longer available. Remove them or refresh content before creating this project."
                            ),
                        );
                    }
                    let query = self.authoring.create.dependency_search.trim().to_lowercase();
                    let mut visible_dependencies = 0;
                    egui::ScrollArea::vertical()
                        .max_height(240.0)
                        .show(ui, |ui| {
                            for content in &self.catalog.sources {
                                let child_kind =
                                    super::super::source_content_kind(&content.kind);
                                let allowed = child_kind.is_some_and(|kind| {
                                    vapor_studio_core::source_dependency_allowed(
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
                                    vapor_studio_core::source_dependency_choice_group(
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
                                    "This occupies one exclusive role slot. Selecting it replaces the current choice for that slot."
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
                                                content.kind,
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
                            ui.weak("No registered content supports this dependency role yet. Create or adopt a compatible project first.");
                        } else {
                            ui.weak("No compatible workspace content matches this search.");
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
                    "Replace {} with {} in this exclusive role slot? Other dependency choices stay selected.",
                    replacement.replacing.join(", "),
                    replacement.target,
                )
            } else {
                format!(
                    "{} is no longer available in the workspace catalog. Cancel this replacement, refresh content, then choose it again.",
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
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.operation.running
                        && !self.has_unsaved_workspace_edits()
                        && !self.authoring.create.pending_dependency_clear
                        && self.authoring.create.pending_role_replacement.is_none(),
                    egui::Button::new("Refresh available content"),
                )
                .on_disabled_hover_text("Save or discard workspace file and structured metadata drafts first; discovery reads saved manifests.")
                .clicked()
            {
                self.discover();
            }
            if ui
                .add_enabled(
                    !self.operation.running
                        && !self.has_unsaved_workspace_edits()
                        && !self.authoring.create.pending_dependency_clear
                        && self.authoring.create.pending_role_replacement.is_none()
                        && unavailable_selected_dependencies == 0
                        && required_role_selected,
                    egui::Button::new("Create content project"),
                )
                .on_disabled_hover_text(if !required_role_selected {
                    "Select the required Engine, Enginepack, Game, or Gamepack role first."
                } else if unavailable_selected_dependencies > 0 {
                    "Remove dependencies that are no longer in the workspace catalog, or refresh available content."
                } else {
                    "Save or discard workspace file and structured metadata drafts before creating content; Studio opens the new manifest afterward."
                })
                .clicked()
            {
                self.create_content_workspace();
            }
        });
        ui.weak("The role manifest is created exclusively. The workspace registration is updated only after the project is written.");
    }
}
