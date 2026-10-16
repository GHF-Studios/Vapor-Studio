use super::super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_create(&mut self, ui: &mut egui::Ui) {
        ui.heading("Create content");
        ui.label("Create a registered Vapor project in the active workspace, then edit its role manifest.");
        ui.weak("Vapor Shell owns the content vocabulary and registers each new project in Workspace.vapor.toml.");
        egui::CollapsingHeader::new("Start from a complete playable workspace")
            .default_open(self.profiles[self.active_profile].workspace.is_none())
            .show(ui, |ui| {
                ui.label("Scaffold an Engine, Game, Packagepack, Cargo projects, and workspace manifests.");
                ui.horizontal(|ui| {
                    ui.label("New folder");
                    ui.text_edit_singleline(&mut self.starter_workspace_path);
                });
                ui.horizontal(|ui| {
                    ui.label("Organization");
                    ui.text_edit_singleline(&mut self.starter_workspace_organization);
                    ui.label("Workspace name");
                    ui.text_edit_singleline(&mut self.starter_workspace_name);
                });
                if ui.add_enabled(!self.running && !self.workspace_editor_dirty, egui::Button::new("Create and open starter workspace"))
                    .on_disabled_hover_text("Save or reload the open file before creating and opening another workspace.")
                    .clicked()
                {
                    self.create_starter_workspace();
                }
                ui.weak("The destination must be empty. Its package metadata uses this Vapor installation's Steam AppID.");
            });
        ui.separator();
        egui::ComboBox::from_id_salt("new-content-kind")
            .selected_text(self.draft_content_kind.as_str())
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
                    ui.selectable_value(&mut self.draft_content_kind, kind, kind.as_str());
                }
            });
        ui.horizontal(|ui| {
            ui.label("Content ID under this workspace's namespace");
            ui.text_edit_singleline(&mut self.draft_content_id);
        });
        ui.horizontal(|ui| {
            ui.label("Version");
            ui.text_edit_singleline(&mut self.draft_content_version);
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.running && !self.workspace_editor_dirty,
                    egui::Button::new("Refresh available content"),
                )
                .on_disabled_hover_text("Save or reload the open file first; discovery reads saved workspace manifests.")
                .clicked()
            {
                self.discover();
            }
            if ui
                .add_enabled(!self.running && !self.workspace_editor_dirty, egui::Button::new("Create content workspace"))
                .on_disabled_hover_text("Save or reload the open file before creating content; Studio opens the new manifest afterward.")
                .clicked()
            {
                self.create_content_workspace();
            }
        });
        ui.separator();
        ui.label(format!("{} contents", self.draft_content_kind.as_str()));
        if self.contents.is_empty() {
            ui.weak("Discover registered workspace content to choose dependencies.");
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for content in &self.contents {
                let id = super::super::source_content_selector(content);
                let child_kind = super::super::source_content_kind(&content.kind);
                let allowed = child_kind.is_some_and(|kind| {
                    vapor_studio_core::source_dependency_allowed(self.draft_content_kind, kind)
                });
                if !allowed {
                    continue;
                }
                let mut included = self.draft_dependencies.iter().any(|value| value == &id);
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
                    .changed()
                {
                    if included {
                        self.draft_dependencies.push(id.to_owned());
                    } else {
                        self.draft_dependencies.retain(|value| value != &id);
                    }
                }
            }
        });
        ui.weak("The role manifest is created exclusively. The workspace registration is updated only after the project is written.");
    }
}
