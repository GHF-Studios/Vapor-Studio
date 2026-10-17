use super::super::StudioApp;
use super::super::components::design::empty_state;
use super::super::content_selector;
use eframe::egui;
use vapor_studio_core::InstallationState;

pub(super) enum ProfileCompositionAction {
    BrowseContent,
    CreateContent,
    EditComposition(String),
    PreviewPlan,
    DownloadDependencies(Vec<String>),
}

impl StudioApp {
    pub(super) fn draw_profile_composition_panel(
        &self,
        ui: &mut eframe::egui::Ui,
        profile: &vapor_studio_core::CompositionProfile,
    ) -> Option<ProfileCompositionAction> {
        let Some(root) = &profile.root_package else {
            let mut action = None;
            empty_state(
                ui,
                "No composition selected",
                "Choose a Packagepack to make this profile playable. It remains separate from the source files in your workspace.",
                |ui| {
                    if ui.button("Browse available content").clicked() {
                        action = Some(ProfileCompositionAction::BrowseContent);
                    }
                    let resume_draft = self.authoring.create.is_dirty();
                    if ui
                        .button(if resume_draft {
                            "Continue content draft"
                        } else {
                            "Create a Packagepack"
                        })
                        .clicked()
                    {
                        action = Some(ProfileCompositionAction::CreateContent);
                    }
                },
            );
            return action;
        };
        let mut action = None;
        let selector = content_selector(root);
        let editable_source = self.catalog.sources.iter().any(|content| {
            content.id == root.id && content.kind == vapor_core::ContentKind::Packagepack.as_str()
        });
        if ui
            .add_enabled(
                !self.operation.running && !self.has_unsaved_workspace_edits() && editable_source,
                egui::Button::new("Edit composition"),
            )
            .on_disabled_hover_text(
                "Only a Packagepack in the active workspace can be edited here.",
            )
            .clicked()
        {
            action = Some(ProfileCompositionAction::EditComposition(selector.clone()));
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
            action = Some(ProfileCompositionAction::PreviewPlan);
        }
        if let Some(preview) = self
            .profile_composition
            .preview()
            .filter(|preview| preview.selector == selector)
        {
            let published_file_ids =
                super::super::components::composition_preview::draw_composition_preview(
                    ui, preview,
                );
            if !published_file_ids.is_empty() {
                action = Some(ProfileCompositionAction::DownloadDependencies(
                    published_file_ids,
                ));
            }
        }

        if let Some(graph) = &profile.resolved_graph {
            ui.group(|ui| {
                ui.label(format!("Selected content · {} items", graph.nodes.len()));
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
                egui::CollapsingHeader::new("Included content")
                    .default_open(true)
                    .show(ui, |ui| {
                        for node in &graph.nodes {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(node.kind.as_str());
                                ui.label(content_selector(&node.content));
                                if !node.dependencies.is_empty() {
                                    ui.weak(format!("{} dependencies", node.dependencies.len()));
                                }
                            });
                        }
                    });
                egui::CollapsingHeader::new("Integrity details").show(ui, |ui| {
                    if let InstallationState::Installed {
                        graph_fingerprint: Some(fingerprint),
                        ..
                    } = &profile.installation
                    {
                        ui.small(format!("Composition fingerprint: {fingerprint}"));
                    }
                    for node in &graph.nodes {
                        ui.small(format!(
                            "{} · {} · source {} · file fingerprint {}",
                            node.kind,
                            content_selector(&node.content),
                            node.source.as_deref().unwrap_or("not recorded"),
                            node.fingerprint
                                .as_deref()
                                .unwrap_or("file fingerprint unavailable"),
                        ));
                    }
                });
            });
        } else {
            ui.weak(
                "Install this composition to record its exact content versions and dependencies.",
            );
        }
        action
    }
}
