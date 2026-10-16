use super::super::{Section, StudioApp};
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_home(&mut self, ui: &mut egui::Ui) {
        ui.heading("Vapor Studio");
        ui.label("Find content, manage launchable compositions, or continue authoring.");
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!self.operation.running && !self.workspace.editor_dirty, egui::Button::new("Discover content"))
                .on_disabled_hover_text("Save or reload the open file first; discovery reads saved workspace manifests.")
                .clicked()
            {
                self.section = Section::Discover;
                self.discover();
            }
            if ui.button("Open library").clicked() { self.section = Section::Library; }
            if ui.button("Compose a Packagepack").clicked() { self.section = Section::Compose; }
            if ui.button("Create content").clicked() { self.section = Section::Create; }
            if ui.button("Open workspace").clicked() { self.section = Section::Workspace; }
        });
        ui.separator();
        ui.heading(&self.profile.items[self.profile.active_index].name);
        if let Some(root) = self.profile.items[self.profile.active_index].root_package.as_ref() {
            let content_id = root
                .version
                .as_deref()
                .map(|version| format!("{}@{version}", root.id))
                .unwrap_or_else(|| root.id.clone());
            ui.label(format!("Selected root: {content_id}"));
            let installed = matches!(
                &self.profile.items[self.profile.active_index].installation,
                vapor_studio_core::InstallationState::Installed { .. },
            );
            let source_ready = !self.workspace.editor_dirty
                && self.profile_composition.preview().is_some_and(|preview| {
                    preview.selector == content_id
                        && preview.ready()
                        && preview.nodes.last().is_some_and(|root| {
                            root.source == vapor_studio_core::CompositionPreviewSource::Workspace
                        })
                });
            let action = if installed {
                "Play installed composition"
            } else if self.workspace.editor_dirty {
                "Save workspace edits before source play"
            } else if source_ready {
                "Build and play source composition"
            } else {
                "Review composition in Library"
            };
            if ui
                .add_enabled(
                    !self.operation.running && (installed || !self.workspace.editor_dirty),
                    egui::Button::new(action),
                )
                .on_disabled_hover_text(
                    "Save or reload the open file before resolving and playing source content.",
                )
                .clicked()
            {
                if installed {
                    self.run_installed_selected();
                } else if source_ready {
                    self.run_selected();
                } else {
                    self.section = Section::Library;
                    self.preview_selected_composition();
                }
            }
        } else {
            ui.weak("No root Packagepack selected for this composition yet.");
            if ui.add_enabled(!self.operation.running && !self.workspace.editor_dirty, egui::Button::new("Choose content"))
                .on_disabled_hover_text("Save or reload the open file first; discovery reads saved workspace manifests.")
                .clicked()
            {
                self.section = Section::Discover;
                self.discover();
            }
        }
        ui.separator();
        ui.label("Recent activity");
        ui.label(&self.status);
    }
}
