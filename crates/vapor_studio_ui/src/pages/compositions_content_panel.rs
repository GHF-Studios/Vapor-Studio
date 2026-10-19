use super::super::StudioApp;
use super::super::components::design::empty_state;
use super::super::content_selector;
use eframe::egui;

pub(super) enum CompositionContentAction {
    BrowseContent,
    CreateContent,
    EditComposition(String),
    PreviewPlan,
    DownloadDependencies(Vec<String>),
}

impl StudioApp {
    pub(super) fn draw_composition_content_panel(
        &self,
        ui: &mut eframe::egui::Ui,
        composition: &vapor_client_application::Composition,
    ) -> Option<CompositionContentAction> {
        let Some(root) = &composition.root_package else {
            let mut action = None;
            empty_state(
                ui,
                "Choose a composition",
                "Choose a complete playable setup, or create one from your projects. Your project files stay where they are.",
                |ui| {
                    if ui.button("Browse compositions").clicked() {
                        action = Some(CompositionContentAction::BrowseContent);
                    }
                    let resume_draft = self.authoring.create.is_dirty();
                    if ui
                        .button(if resume_draft {
                            "Continue content draft"
                        } else {
                            "Create a composition"
                        })
                        .clicked()
                    {
                        action = Some(CompositionContentAction::CreateContent);
                    }
                },
            );
            return action;
        };
        let mut action = None;
        let selector = content_selector(root);
        let editable_source = self.catalog.sources.iter().any(|content| {
            content.id == root.id
                && content.kind == vapor_client_protocol::ContentKind::Packagepack.as_str()
        });
        if ui
            .add_enabled(
                !self.operation.running && !self.has_unsaved_workspace_edits() && editable_source,
                egui::Button::new("Edit composition"),
            )
            .on_disabled_hover_text(
                "Only a Packagepack project in an open project folder can be edited here.",
            )
            .clicked()
        {
            action = Some(CompositionContentAction::EditComposition(selector.clone()));
        }
        if ui
            .add_enabled(
                !self.operation.running && !self.has_unsaved_workspace_edits(),
                egui::Button::new("Review composition"),
            )
            .on_disabled_hover_text(
                "Save or discard your open project edits first; Studio reads the saved project details.",
            )
            .clicked()
        {
            action = Some(CompositionContentAction::PreviewPlan);
        }
        if let Some(preview) = self
            .composition_preview
            .preview()
            .filter(|preview| preview.selector == selector)
        {
            let published_file_ids =
                super::super::components::composition_preview::draw_composition_preview(
                    ui,
                    preview,
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                    &self.catalog.items,
                );
            if !published_file_ids.is_empty() {
                action = Some(CompositionContentAction::DownloadDependencies(
                    published_file_ids,
                ));
            }
        }

        if let Some(graph) = &composition.resolved_graph {
            ui.group(|ui| {
                ui.label(format!("This setup contains {} items", graph.nodes.len()));
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Engine: {}",
                        super::super::display_catalog_identity(
                            &graph.effective_engine.id,
                            graph.effective_engine.version.as_deref(),
                            &self.catalog.items,
                        )
                    ));
                    ui.label(format!(
                        "Game: {}",
                        graph
                            .effective_game
                            .as_ref()
                            .map(|identity| {
                                super::super::display_catalog_identity(
                                    &identity.id,
                                    identity.version.as_deref(),
                                    &self.catalog.items,
                                )
                            })
                            .unwrap_or_else(|| "None".to_owned())
                    ));
                });
                egui::CollapsingHeader::new("Show included items")
                    .default_open(false)
                    .show(ui, |ui| {
                        for node in &graph.nodes {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(super::super::display_content_kind(node.kind.as_str()));
                                ui.label(super::super::display_catalog_identity(
                                    &node.content.id,
                                    node.content.version.as_deref(),
                                    &self.catalog.items,
                                ));
                                if !node.dependencies.is_empty() {
                                    ui.weak(format!("{} dependencies", node.dependencies.len()));
                                }
                            });
                        }
                    });
            });
        } else {
            ui.weak("Install this composition to see its included content and launch status.");
        }
        action
    }
}
