use super::super::StudioApp;
use eframe::egui;
use std::path::PathBuf;
use vapor_studio_core::SourceCompositionLink;

impl StudioApp {
    pub(in crate::app) fn draw_composition(&mut self, ui: &mut egui::Ui) {
        ui.heading("Compose source content");
        ui.label("Edit the dependency graph of a locally authored content project.");
        ui.weak("Vapor Shell owns validation and manifest writes. Studio shows the current source snapshot and the exact graph you are changing.");

        let source_projects = self
            .contents
            .iter()
            .map(|content| {
                (
                    super::super::source_content_selector(content),
                    content.kind.clone(),
                    content.version.clone(),
                )
            })
            .collect::<Vec<_>>();
        ui.horizontal(|ui| {
            ui.label("Source project");
            egui::ComboBox::from_id_salt("composition-source-project")
                .selected_text(if self.composition_edit_selector.is_empty() {
                    "Select content".to_owned()
                } else {
                    self.composition_edit_selector.clone()
                })
                .show_ui(ui, |ui| {
                    for (selector, kind, _) in &source_projects {
                        if ui
                            .selectable_label(
                                self.composition_edit_selector == *selector,
                                format!("{selector} · {kind}"),
                            )
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
                    egui::Button::new("Load dependencies"),
                )
                .clicked()
            {
                self.load_source_composition();
            }
            if ui
                .add_enabled(!self.running && self.composition_edit.is_some(), egui::Button::new("Save dependencies"))
                .clicked()
            {
                self.save_source_composition();
            }
        });

        if source_projects.is_empty() {
            ui.weak("No local content projects found. Open a workspace and discover its content first.");
        }

        let mut remove_link = None;
        let mut use_profile_root = false;
        let mut preview_source_selector = None;
        let mut stage_source_selector = None;
        let mut source_graph_saved = false;
        if let Some(composition) = self.composition_edit.as_mut() {
            ui.separator();
            ui.label(format!("Editing {} · {}", composition.id, composition.kind));
            ui.small(format!("Manifest: {}", composition.manifest.display()));
            ui.label("Current edges");
            if composition.links.is_empty() {
                ui.weak("This project currently has no dependency edges. Vapor Shell enforces role-specific requirements when saving.");
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
                    match (
                        super::super::source_content_kind(&composition.kind),
                        super::super::source_content_kind(&content.kind),
                    ) {
                        (Some(parent), Some(child)) => vapor_studio_core::source_dependency_relationship(parent, child).is_some(),
                        _ => false,
                    }
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
                        let relationship = super::super::source_content_kind(&composition.kind)
                            .and_then(|parent| super::super::source_content_kind(kind)
                                .and_then(|child| vapor_studio_core::source_dependency_relationship(parent, child)))
                            .unwrap_or("dependency");
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
            source_graph_saved = self.composition_original_links.as_ref() == Some(&composition.links);
            if composition.kind == vapor_core::ContentKind::Packagepack.as_str() {
                ui.horizontal_wrapped(|ui| {
                    let source_selector = composition.version.as_ref()
                        .map(|version| format!("{}@{version}", composition.id))
                        .unwrap_or_else(|| composition.id.clone());
                    if ui.add_enabled(!self.running && source_graph_saved, egui::Button::new("Preview availability")).clicked() {
                        preview_source_selector = Some(source_selector);
                    }
                    if ui.add_enabled(!self.running && source_graph_saved, egui::Button::new("Use as active profile root"))
                        .on_disabled_hover_text("Save the current graph before resolving it into a profile.")
                        .clicked()
                    {
                        use_profile_root = true;
                    }
                    ui.weak("This selects the source Packagepack and refreshes its resolved plan after save.");
                });
            }
            if ui
                .add_enabled(
                    !self.running && source_graph_saved,
                    egui::Button::new("Stage Workshop review"),
                )
                .on_disabled_hover_text("Save the current graph before staging its source content for review.")
                .clicked()
            {
                stage_source_selector = Some(
                    composition.version.as_ref()
                        .map(|version| format!("{}@{version}", composition.id))
                        .unwrap_or_else(|| composition.id.clone()),
                );
            }
        } else {
            ui.separator();
            ui.weak("Load a source content project to inspect and edit its dependencies. Cache and installed copies remain read-only here.");
        }
        if let Some(selector) = preview_source_selector {
            self.preview_source_composition(selector);
        }
        if use_profile_root {
            self.use_edited_composition_for_profile();
        }
        if let Some(selector) = stage_source_selector {
            self.selected_catalog_content = Some(selector.clone());
            self.section = super::super::Section::Discover;
            self.stage_workshop_content(selector);
        }
        let selector = self.composition_edit.as_ref().map(|composition| {
            composition.version.as_ref()
                .map(|version| format!("{}@{version}", composition.id))
                .unwrap_or_else(|| composition.id.clone())
        });
        let preview = self.composition_preview.clone();
        if source_graph_saved
            && let (Some(selector), Some(preview)) = (selector, preview)
            && preview.selector == selector
        {
            if let Some(published_file_id) =
                super::super::components::composition_preview::draw_composition_preview(
                    ui, &preview,
                )
            {
                self.workshop_id = published_file_id;
                self.section = super::super::Section::Discover;
                self.download_workshop_item();
            }
        }
        ui.add_space(6.0);
        ui.small(format!("Workspace: {}", PathBuf::from(self.workspace_root.trim()).display()));
    }
}
