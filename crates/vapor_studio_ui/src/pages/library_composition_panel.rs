use super::super::StudioApp;
use super::super::content_selector;
use super::super::components::design::empty_state;
use eframe::egui;
use vapor_studio_core::InstallationState;

impl StudioApp {
    pub(super) fn draw_profile_composition_panel(
        &mut self,
        ui: &mut eframe::egui::Ui,
        profile: &vapor_studio_core::CompositionProfile,
    ) {
        let Some(root) = &profile.root_package else {
            empty_state(
                ui,
                "No composition selected",
                "Choose a Packagepack to make this profile playable. It remains separate from the source files in your workspace.",
                |ui| {
                    if ui.button("Browse available content").clicked() {
                        self.section = super::super::Section::Discover;
                    }
                    if ui.button("Create a Packagepack").clicked() {
                        self.section = super::super::Section::Create;
                        self.authoring.draft_content_kind = vapor_core::ContentKind::Packagepack;
                    }
                },
            );
            return;
        };
        {
            let selector = content_selector(root);
            let editable_source = self.catalog.sources.iter().any(|content| {
                content.id == root.id
                    && content.kind == vapor_core::ContentKind::Packagepack.as_str()
            });
            if ui
                .add_enabled(
                    !self.operation.running
                        && !self.has_unsaved_workspace_edits()
                        && editable_source,
                    egui::Button::new("Edit source graph"),
                )
                .on_disabled_hover_text(
                    "Only a Packagepack authored in the active workspace can be edited here.",
                )
                .clicked()
            {
                self.authoring.composition_edit_selector = selector.clone();
                self.authoring.composition_edit = None;
                self.authoring.composition_original_links = None;
                self.section = super::super::Section::Compose;
                self.load_source_composition();
            }
            if ui
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                    egui::Button::new("Preview dependency plan"),
                )
                .on_disabled_hover_text(
                    "Save or discard workspace file and structured metadata drafts first; resolution reads manifests from disk.",
                )
                .clicked()
            {
                self.preview_selected_composition();
            }
            let preview = self
                .profile_composition
                .preview()
                .filter(|preview| preview.selector == selector)
                .cloned();
            if let Some(preview) = preview {
                let published_file_ids =
                    super::super::components::composition_preview::draw_composition_preview(
                        ui, &preview,
                    );
                if !published_file_ids.is_empty() {
                    self.download_profile_composition_dependencies(published_file_ids);
                }
            }
        }

        if let Some(graph) = &profile.resolved_graph {
            ui.group(|ui| {
                ui.label(format!(
                    "Resolved composition · {} content nodes",
                    graph.nodes.len()
                ));
                if let InstallationState::Installed {
                    graph_fingerprint: Some(fingerprint),
                    ..
                } = &profile.installation
                {
                    ui.small(format!("Resolved graph fingerprint: {fingerprint}"));
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Engine: {}",
                        content_selector(&graph.effective_engine)
                    ));
                    ui.label(format!(
                        "Game: {}",
                        graph
                            .effective_game
                            .as_ref()
                            .map(content_selector)
                            .unwrap_or_else(|| "None".to_owned())
                    ));
                });
                egui::CollapsingHeader::new("Resolved content")
                    .default_open(true)
                    .show(ui, |ui| {
                        for node in &graph.nodes {
                            ui.horizontal(|ui| {
                                ui.label(node.kind.as_str());
                                ui.label(content_selector(&node.content));
                                ui.weak(format!(
                                    "Payload {}",
                                    node.fingerprint.as_deref().unwrap_or("not fingerprinted")
                                ));
                                if let Some(source) = &node.source {
                                    ui.weak(format!("Installed from {source}"));
                                }
                                if !node.dependencies.is_empty() {
                                    ui.weak(format!("{} dependencies", node.dependencies.len()));
                                }
                            });
                        }
                    });
            });
        } else {
            ui.weak("Install this composition to resolve and record its exact dependency graph.");
        }
    }
}
