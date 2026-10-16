use super::super::StudioApp;
use super::super::content_selector;
use eframe::egui;
use vapor_studio_core::InstallationState;

impl StudioApp {
    pub(super) fn draw_profile_installation_panel(
        &mut self,
        ui: &mut eframe::egui::Ui,
        profile: &vapor_studio_core::CompositionProfile,
    ) {
        ui.horizontal_wrapped(|ui| {
            let has_root = profile.root_package.is_some();
            let has_source = profile.workspace.is_some();
            let root_plan_node = profile.root_package.as_ref().and_then(|root| {
                let selector = content_selector(root);
                self.profile_composition.preview()
                    .filter(|preview| preview.selector == selector && preview.ready())
                    .and_then(|preview| preview.nodes.last())
            });
            let preview_ready = root_plan_node.is_some();
            let root_installed = root_plan_node.is_some_and(|node| node.already_installed);
            let root_has_refresh_source = root_plan_node.is_some_and(|node| {
                node.source != vapor_studio_core::CompositionPreviewSource::Installed
            });
            let root_is_workspace_source = root_plan_node.is_some_and(|node| {
                node.source == vapor_studio_core::CompositionPreviewSource::Workspace
            });
            let installed = matches!(&profile.installation, InstallationState::Installed { .. });
            let needs_repair = matches!(&profile.installation,
                InstallationState::NeedsRepair { .. } | InstallationState::Partial { .. });
            if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && has_root && has_source, egui::Button::new("Validate source"))
                .on_disabled_hover_text("Source validation needs this profile to be associated with a workspace. Cached or seeded compositions validate during install.")
                .clicked() { self.validate_selected_source(); }
            if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && has_root && preview_ready && !installed && !root_installed && root_has_refresh_source, egui::Button::new("Install composition"))
                .on_disabled_hover_text("Preview a ready plan with an installable workspace or cache root. An already installed root needs verification, playback, or an update.")
                .clicked() { self.install_selected(); }
            if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && has_root && installed && preview_ready && root_has_refresh_source, egui::Button::new("Update composition"))
                .on_disabled_hover_text("Update uses the ready plan's current workspace or cached sources.")
                .clicked() { self.update_selected(); }
            if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && has_root && has_source && root_is_workspace_source, egui::Button::new("Build and play source"))
                .on_disabled_hover_text("The selected Packagepack must resolve from this workspace before source build/play is available.")
                .clicked() { self.run_selected(); }
            if ui.add_enabled(!self.operation.running && has_root && (installed || root_installed), egui::Button::new("Play installed")).clicked() { self.run_installed_selected(); }
            if ui.add_enabled(!self.operation.running && has_root && (installed || root_installed), egui::Button::new("Verify graph")).clicked() { self.verify_selected_installation(); }
            if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && has_root && preview_ready && root_has_refresh_source && needs_repair, egui::Button::new("Repair"))
                .on_disabled_hover_text("Repair needs a ready workspace or cached source to replace damaged installed files.")
                .clicked() { self.repair_selected_installation(); }
        });

        ui.label("Installation");
        match &profile.installation {
            InstallationState::Unknown => {
                ui.weak("Not checked")
                    .on_hover_text("Studio has not inspected this profile's installed content.");
            }
            InstallationState::NotInstalled => {
                ui.weak("Not installed");
            }
            InstallationState::Installed { artifacts, .. } => {
                ui.label(format!(
                    "{} exact content versions installed",
                    artifacts.len()
                ));
                for artifact in artifacts {
                    ui.small(format!(
                        "{}{} · {}",
                        artifact.id,
                        artifact
                            .version
                            .as_ref()
                            .map(|version| format!("@{version}"))
                            .unwrap_or_default(),
                        artifact.root.display()
                    ));
                }
            }
            InstallationState::Partial { detail } => {
                ui.colored_label(egui::Color32::YELLOW, "Installation needs attention");
                ui.label(detail);
            }
            InstallationState::NeedsRepair { detail, artifacts } => {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("{} installed versions need repair", artifacts.len()),
                );
                ui.label(detail);
            }
        }

        ui.separator();
        if ui
            .add_enabled(
                !self.operation.running && !self.has_unsaved_workspace_edits(),
                egui::Button::new("Discover content"),
            )
            .on_disabled_hover_text(
                "Save or discard workspace file and structured metadata drafts first; discovery reads saved manifests.",
            )
            .clicked()
        {
            self.section = super::super::Section::Discover;
            self.discover();
        }
    }
}
