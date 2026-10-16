use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_create(&mut self, ui: &mut egui::Ui) {
        ui.heading("Create content");
        ui.label("Create a registered Vapor project in the active workspace, then edit its role manifest.");
        ui.weak("Vapor Shell owns the content vocabulary and registers each new project in Workspace.vapor.toml.");
        egui::CollapsingHeader::new("Start from a complete playable workspace")
            .default_open(self.profile.items[self.profile.active_index].workspace.is_none())
            .show(ui, |ui| {
                ui.label("Scaffold an Engine, Game, Packagepack, Cargo projects, and workspace manifests.");
                ui.horizontal(|ui| {
                    ui.label("New folder");
                    ui.text_edit_singleline(&mut self.workspace.starter_path);
                });
                ui.horizontal(|ui| {
                    ui.label("Organization");
                    ui.text_edit_singleline(&mut self.workspace.starter_organization);
                    ui.label("Workspace name");
                    ui.text_edit_singleline(&mut self.workspace.starter_name);
                });
                if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Create and open starter workspace"))
                    .on_disabled_hover_text("Save or discard workspace file and structured metadata drafts before creating and opening another workspace.")
                    .clicked()
                {
                    self.create_starter_workspace();
                }
                ui.weak("The destination must be empty. Its package metadata uses this Vapor installation's Steam AppID.");
            });
        ui.separator();
        let mut content_kind_changed = false;
        egui::ComboBox::from_id_salt("new-content-kind")
            .selected_text(self.authoring.draft_content_kind.as_str())
            .show_ui(ui, |ui| {
                for kind in [
                    vapor_core::ContentKind::Packagepack,
                    vapor_core::ContentKind::Enginepack,
                    vapor_core::ContentKind::Gamepack,
                    vapor_core::ContentKind::Modpack,
                    vapor_core::ContentKind::Engine,
                    vapor_core::ContentKind::Game,
                    vapor_core::ContentKind::EngineMod,
                    vapor_core::ContentKind::GameMod,
                    vapor_core::ContentKind::ExtensionMod,
                ] {
                    content_kind_changed |= ui
                        .selectable_value(&mut self.authoring.draft_content_kind, kind, kind.as_str())
                        .changed();
                }
            });
        if content_kind_changed {
            self.authoring.draft_dependencies.clear();
        }
        let role_constraints = match self.authoring.draft_content_kind {
            vapor_core::ContentKind::Packagepack => "Choose exactly one Engine or Enginepack; optionally choose at most one Game or Gamepack.",
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
        let required_role_group =
            vapor_studio_core::source_required_dependency_choice_group(self.authoring.draft_content_kind);
        let required_role_selected = required_role_group.is_none_or(|required_group| {
            self.catalog.sources.iter().any(|content| {
                self.authoring.draft_dependencies
                    .iter()
                    .any(|selected| selected == super::super::source_content_selector(content))
                    && super::super::source_content_kind(&content.kind).is_some_and(|kind| {
                        vapor_studio_core::source_dependency_choice_group(
                            self.authoring.draft_content_kind,
                            kind,
                        ) == Some(required_group)
                    })
            })
        });
        ui.weak(role_constraints);
        ui.small(source_shape);
        ui.horizontal(|ui| {
            ui.label("Content ID under this workspace's namespace");
            ui.text_edit_singleline(&mut self.authoring.draft_content_id);
        });
        ui.horizontal(|ui| {
            ui.label("Version");
            ui.text_edit_singleline(&mut self.authoring.draft_content_version);
        });
        egui::CollapsingHeader::new("Workshop publication metadata · optional")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("These fields are authored into the role manifest. Creating content never publishes it.");
                ui.horizontal(|ui| {
                    ui.label("Steam AppID");
                    ui.text_edit_singleline(&mut self.authoring.draft_workshop_app_id);
                    ui.label("Existing PublishedFileId");
                    ui.text_edit_singleline(&mut self.authoring.draft_workshop_published_file_id);
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
                        && !self.workspace.root.trim().is_empty()
                        && required_role_selected,
                    egui::Button::new("Create content project"),
                )
                .on_disabled_hover_text(if self.workspace.root.trim().is_empty() {
                    "Open a Vapor Workspace before creating content."
                } else if !required_role_selected {
                    "Select the required Engine, Enginepack, Game, or Gamepack role first."
                } else {
                    "Save or discard workspace file and structured metadata drafts before creating content; Studio opens the new manifest afterward."
                })
                .clicked()
            {
                self.create_content_workspace();
            }
        });
        ui.separator();
        ui.label(format!("{} contents", self.authoring.draft_content_kind.as_str()));
        if self.catalog.sources.is_empty() {
            ui.weak("Discover registered workspace content to choose dependencies.");
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
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
                                            .any(|selected| selected == candidate_id)
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
        ui.weak("The role manifest is created exclusively. The workspace registration is updated only after the project is written.");
    }
}
