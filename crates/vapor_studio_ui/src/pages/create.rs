use super::super::StudioApp;
use super::super::components::design::{ACCENT, page_header};
use eframe::egui;
use std::path::Path;

impl StudioApp {
    pub(in crate::app) fn draw_create(&mut self, ui: &mut egui::Ui) {
        let operation_running = self.operation.running;
        ui.add_enabled_ui(!operation_running, |ui| self.draw_create_content(ui));
    }

    fn draw_create_content(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Author", "Create", "Start a content project from a template, then add its identity and required relationships.");
        if self.authoring.create_draft_is_dirty()
            && ui.button("Discard current content draft").clicked()
        {
            self.authoring.reset_create_draft();
            self.status = "Discarded the new content project draft.".to_owned();
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
                ui.horizontal(|ui| {
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
                self.section = super::super::Section::Workspace;
            }
        });

        if !self.authoring.create_details_open {
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
                for group in ["PACKS", "RUNTIME", "EXTENSIONS"] {
                    ui.small(group);
                    ui.horizontal_wrapped(|ui| {
                        for (_, kind, title, description) in templates.iter().filter(|(family, ..)| *family == group) {
                            egui::Frame::group(&card_style).show(ui, |ui| {
                                ui.set_min_width(175.0);
                                ui.set_max_width(220.0);
                                let selected = self.authoring.draft_content_kind == *kind;
                                if ui.add_sized(
                                    [ui.available_width(), 34.0],
                                    egui::Button::selectable(selected, egui::RichText::new(*title).strong()),
                                ).clicked() && !selected {
                                    let cleared_dependencies =
                                        !self.authoring.draft_dependencies.is_empty();
                                    self.authoring.draft_content_kind = *kind;
                                    self.authoring.draft_dependencies.clear();
                                    if cleared_dependencies {
                                        self.status = "Changed the content template and cleared its dependency selections.".to_owned();
                                    }
                                }
                                ui.small(*description);
                            });
                        }
                    });
                }
                let description = match self.authoring.draft_content_kind {
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
                    if ui.add_enabled(!self.operation.running, egui::Button::new("Continue")).clicked() {
                        self.authoring.create_details_open = true;
                    }
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Refresh workspace content")).clicked() {
                        self.discover();
                    }
                });
            });
            return;
        }

        ui.horizontal(|ui| {
            if ui.small_button("← Templates").clicked() {
                self.authoring.create_details_open = false;
            }
            ui.label(egui::RichText::new("STEP 2 OF 2 · DETAILS").small().strong().color(ACCENT));
            ui.heading(format!("New {} project", self.authoring.draft_content_kind));
        });
        ui.label("Set identity and required dependencies. Workshop details are optional and can be added later.");
        ui.separator();
        let role_constraints = match self.authoring.draft_content_kind {
            vapor_core::ContentKind::Packagepack => {
                "Choose exactly one Engine or Enginepack; optionally choose at most one Game or Gamepack."
            }
            vapor_core::ContentKind::Game => "Choose exactly one Engine.",
            vapor_core::ContentKind::Enginepack => "Choose exactly one Engine.",
            vapor_core::ContentKind::Gamepack => "Choose exactly one Game.",
            _ => "Dependencies are checked against this role's supported content kinds.",
        };
        let source_shape = match self.authoring.draft_content_kind {
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
            self.authoring.draft_content_kind,
        );
        let required_role_selected = required_role_group.is_none_or(|required_group| {
            self.catalog.sources.iter().any(|content| {
                self.authoring
                    .draft_dependencies
                    .iter()
                    .any(|selected| selected == &super::super::source_content_selector(content))
                    && super::super::source_content_kind(&content.kind).is_some_and(|kind| {
                        vapor_studio_core::source_dependency_choice_group(
                            self.authoring.draft_content_kind,
                            kind,
                        ) == Some(required_group)
                    })
            })
        });
        ui.small(source_shape);
        egui::Grid::new("new-content-identity")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Content ID");
                ui.add(egui::TextEdit::singleline(&mut self.authoring.draft_content_id).desired_width(320.0));
                ui.end_row();
                ui.label("Version");
                ui.add(egui::TextEdit::singleline(&mut self.authoring.draft_content_version).desired_width(180.0));
                ui.end_row();
            });
        egui::CollapsingHeader::new("Workshop publication metadata · optional")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("These fields are authored into the role manifest. Creating content never publishes it.");
                egui::Grid::new("new-content-workshop-identity")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Steam AppID");
                        ui.add(egui::TextEdit::singleline(&mut self.authoring.draft_workshop_app_id).desired_width(180.0));
                        ui.end_row();
                        ui.label("PublishedFileId");
                        ui.add(egui::TextEdit::singleline(&mut self.authoring.draft_workshop_published_file_id).desired_width(320.0));
                        ui.end_row();
                    });
                ui.horizontal(|ui| {
                    ui.label("Visibility");
                    egui::ComboBox::from_id_salt("new-content-workshop-visibility")
                        .selected_text(if self.authoring.draft_workshop_visibility.is_empty() {
                            "Not set"
                        } else {
                            &self.authoring.draft_workshop_visibility
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.authoring.draft_workshop_visibility, String::new(), "Not set");
                            for visibility in ["private", "friends-only", "public"] {
                                ui.selectable_value(
                                    &mut self.authoring.draft_workshop_visibility,
                                    visibility.to_owned(),
                                    visibility,
                                );
                            }
                        });
                    ui.label("Title");
                    ui.text_edit_singleline(&mut self.authoring.draft_workshop_title);
                });
                ui.label("Description");
                ui.add(
                    egui::TextEdit::multiline(&mut self.authoring.draft_workshop_description)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    ui.label("Tags · comma separated");
                    ui.text_edit_singleline(&mut self.authoring.draft_workshop_tags);
                    ui.label("Change note");
                    ui.text_edit_singleline(&mut self.authoring.draft_workshop_change_note);
                });
            });
        egui::CollapsingHeader::new("Dependencies")
            .default_open(true)
            .show(ui, |ui| {
                ui.weak(role_constraints);
                ui.horizontal(|ui| {
                    ui.label("Selected dependencies");
                    if ui
                        .add_enabled(
                            !self.authoring.draft_dependencies.is_empty(),
                            egui::Button::new("Clear selection"),
                        )
                        .clicked()
                    {
                        self.authoring.draft_dependencies.clear();
                        self.status = "Cleared the new content project's dependency selections.".to_owned();
                    }
                });
                if self.catalog.sources.is_empty() {
                    ui.weak("Refresh workspace content to choose dependencies.");
                }
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
            for content in &self.catalog.sources {
                let id = super::super::source_content_selector(content);
                let child_kind = super::super::source_content_kind(&content.kind);
                let allowed = child_kind.is_some_and(|kind| {
                    vapor_studio_core::source_dependency_allowed(self.authoring.draft_content_kind, kind)
                });
                if !allowed {
                    continue;
                }
                let role_group = child_kind.and_then(|kind| {
                    vapor_studio_core::source_dependency_choice_group(
                        self.authoring.draft_content_kind,
                        kind,
                    )
                });
                let mut included = self.authoring.draft_dependencies.iter().any(|value| value == &id);
                let choice_help = if role_group.is_some() {
                    "This occupies one exclusive role slot. Selecting it replaces the current choice for that slot."
                } else {
                    "Select any number of supported dependencies."
                };
                if ui
                    .checkbox(
                        &mut included,
                        format!(
                            "{} · {} · {}",
                            id,
                            content.kind,
                            content.version.as_deref().unwrap_or("unversioned")
                        ),
                    )
                    .on_hover_text(choice_help)
                    .changed()
                {
                    if included {
                        if let Some(role_group) = role_group {
                            let old_role_choices = self
                                .catalog
                                .sources
                                .iter()
                                .filter(|candidate| {
                                    let candidate_id =
                                        super::super::source_content_selector(candidate);
                                    let Some(candidate_kind) =
                                        super::super::source_content_kind(&candidate.kind)
                                    else {
                                        return false;
                                    };
                                    let candidate_group = vapor_studio_core::source_dependency_choice_group(
                                        self.authoring.draft_content_kind,
                                        candidate_kind,
                                    );
                                    candidate_group == Some(role_group)
                                        && self
                                            .authoring
                                            .draft_dependencies
                                            .iter()
                                            .any(|selected| selected == &candidate_id)
                                })
                                .map(|candidate| {
                                    super::super::source_content_selector(candidate).to_owned()
                                })
                                .collect::<Vec<_>>();
                            self.authoring.draft_dependencies.retain(|value| {
                                !old_role_choices.iter().any(|old| old == value)
                            });
                        }
                        self.authoring.draft_dependencies.push(id.to_owned());
                    } else {
                        self.authoring.draft_dependencies.retain(|value| value != &id);
                    }
                }
            }
                });
            });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
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
                        && required_role_selected,
                    egui::Button::new("Create content project"),
                )
                .on_disabled_hover_text(if !required_role_selected {
                    "Select the required Engine, Enginepack, Game, or Gamepack role first."
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
