use super::super::StudioApp;
use eframe::egui;
use std::path::PathBuf;
use vapor_studio_core::SourceCompositionLink;

impl StudioApp {
    pub(in crate::app) fn draw_composition(&mut self, ui: &mut egui::Ui) {
        ui.heading("Compose a Packagepack");
        ui.label("Edit the dependency graph of a locally authored Packagepack.");
        ui.weak("Vapor Shell owns validation and manifest writes. Studio shows the current source snapshot and the exact graph you are changing.");

        let packagepacks = self
            .contents
            .iter()
            .filter(|content| content.kind == vapor_core::ContentKind::Packagepack.as_str())
            .map(|content| (super::super::source_content_selector(content), content.version.clone()))
            .collect::<Vec<_>>();
        ui.horizontal(|ui| {
            ui.label("Source Packagepack");
            egui::ComboBox::from_id_salt("composition-source-packagepack")
                .selected_text(if self.composition_edit_selector.is_empty() {
                    "Select a Packagepack".to_owned()
                } else {
                    self.composition_edit_selector.clone()
                })
                .show_ui(ui, |ui| {
                    for (selector, _) in &packagepacks {
                        if ui
                            .selectable_label(self.composition_edit_selector == *selector, selector)
                            .clicked()
                        {
                            self.composition_edit_selector = selector.clone();
                            self.composition_edit = None;
                            self.composition_original_links = None;
                        }
                    }
                });
            if ui
                .add_enabled(
                    !self.running
                        && !self.workspace_editor_dirty
                        && !self.composition_edit_selector.trim().is_empty(),
                    egui::Button::new("Load graph"),
                )
                .clicked()
            {
                self.load_source_composition();
            }
            if ui
                .add_enabled(!self.running && self.composition_edit.is_some(), egui::Button::new("Save graph"))
                .clicked()
            {
                self.save_source_composition();
            }
        });

        if packagepacks.is_empty() {
            ui.weak("No local Packagepacks found. Open a workspace and discover its content first.");
        }

        let mut remove_link = None;
        let mut use_profile_root = false;
        if let Some(composition) = self.composition_edit.as_mut() {
            ui.separator();
            ui.label(format!("Editing {}", composition.id));
            ui.small(format!("Manifest: {}", composition.manifest.display()));
            ui.label("Current edges");
            if composition.links.is_empty() {
                ui.weak("This graph has no edges yet. A valid Packagepack needs exactly one Engine or Enginepack.");
            }
            egui::ScrollArea::horizontal().show(ui, |ui| {
                egui::Grid::new("composition-edges")
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Role");
                        ui.strong("Content");
                        ui.strong("Version requirement");
                        ui.strong("Optional");
                        ui.strong("Reason");
                        ui.end_row();
                        for (index, link) in composition.links.iter_mut().enumerate() {
                            ui.label(&link.relationship);
                            ui.label(&link.id);
                            let mut version = link.version.clone().unwrap_or_default();
                            if ui
                                .add(egui::TextEdit::singleline(&mut version).desired_width(150.0).hint_text("any compatible version"))
                                .changed()
                            {
                                link.version = (!version.trim().is_empty()).then_some(version);
                            }
                            ui.checkbox(&mut link.optional, "");
                            let mut reason = link.reason.clone().unwrap_or_default();
                            if ui
                                .add(egui::TextEdit::singleline(&mut reason).desired_width(180.0).hint_text("Why is this included?"))
                                .changed()
                            {
                                link.reason = (!reason.trim().is_empty()).then_some(reason);
                            }
                            if ui.small_button("Remove").clicked() {
                                remove_link = Some(index);
                            }
                            ui.end_row();
                        }
                    });
            });
            if let Some(index) = remove_link {
                composition.links.remove(index);
            }

            ui.separator();
            ui.label("Add a local workspace content edge");
            let choices = self
                .contents
                .iter()
                .filter(|content| content.id != composition.id)
                .filter(|content| {
                    super::super::source_content_kind(&content.kind).is_some_and(|kind| {
                        vapor_studio_core::source_dependency_allowed(
                            vapor_core::ContentKind::Packagepack,
                            kind,
                        )
                    })
                })
                .map(|content| {
                    (
                        super::super::source_content_selector(content),
                        content.kind.clone(),
                        content.version.clone(),
                    )
                })
                .collect::<Vec<_>>();
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("composition-link-candidate")
                    .selected_text(if self.composition_link_candidate.is_empty() {
                        "Choose local content".to_owned()
                    } else {
                        self.composition_link_candidate.clone()
                    })
                    .show_ui(ui, |ui| {
                        for (selector, kind, version) in &choices {
                            let label = format!("{selector} · {kind} · {}", version.as_deref().unwrap_or("unversioned"));
                            ui.selectable_value(
                                &mut self.composition_link_candidate,
                                selector.clone(),
                                label,
                            );
                        }
                    });
                if ui
                    .add_enabled(
                        !self.running
                            && !self.composition_link_candidate.is_empty()
                            && choices.iter().any(|(selector, _, _)| selector == &self.composition_link_candidate),
                        egui::Button::new("Add edge"),
                    )
                    .clicked()
                {
                    if let Some((selector, kind, version)) = choices
                        .iter()
                        .find(|(selector, _, _)| selector == &self.composition_link_candidate)
                    {
                        let id = selector.split('@').next().unwrap_or(selector).to_owned();
                        let relationship = match kind.as_str() {
                            "engine" => "engine",
                            "enginepack" => "enginepack",
                            "game" => "game",
                            "gamepack" => "gamepack",
                            _ => "dependency",
                        };
                        if composition.links.iter().any(|link| link.id == id && link.relationship == relationship) {
                            self.status = format!("{id} is already in this graph.");
                        } else {
                            composition.links.push(SourceCompositionLink {
                                id,
                                relationship: relationship.to_owned(),
                                optional: false,
                                version: version.as_ref().map(|version| format!("={version}")),
                                workshop_id: None,
                                reason: None,
                            });
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                let graph_saved = self.composition_original_links.as_ref() == Some(&composition.links);
                if ui.add_enabled(!self.running && graph_saved, egui::Button::new("Use as active profile root"))
                    .on_disabled_hover_text("Save the current graph before resolving it into a profile.")
                    .clicked()
                {
                    use_profile_root = true;
                }
                ui.weak("This selects the source Packagepack and refreshes its resolved plan after save.");
            });
        } else {
            ui.separator();
            ui.weak("Load a source Packagepack to inspect and edit its graph. Cache and installed copies remain read-only here.");
        }
        if use_profile_root {
            self.use_edited_composition_for_profile();
        }
        ui.add_space(6.0);
        ui.small(format!("Workspace: {}", PathBuf::from(self.workspace_root.trim()).display()));
    }
}
