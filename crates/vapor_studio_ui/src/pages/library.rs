use super::super::StudioApp;
use super::super::components::design::{page_header, section_header, use_stacked_panels};
use super::library_composition_panel::ProfileCompositionAction;
use super::library_installation_panel::ProfileInstallationAction;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_library(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            "Profiles",
            "Your profiles",
            "Each profile keeps one game setup and its launch options. Switch profiles to keep separate setups for different games or add-ons.",
        );
        let profile_index = self.profile.active_index;
        let Some(profile_name) = self
            .profile
            .items
            .get(profile_index)
            .map(|profile| profile.name.clone())
        else {
            return;
        };
        let mut composition_action = None;
        let mut installation_action = None;
        if use_stacked_panels(ui) {
            section_header(
                ui,
                "Saved profiles",
                "A profile remembers one game setup and its launch options.",
            );
            if ui
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_context_switch_edits(),
                    egui::Button::new("Make a copy of this profile"),
                )
                .on_disabled_hover_text(
                    "Discard the Create draft or save/discard workspace edits before duplicating a profile.",
                )
                .clicked()
            {
                self.duplicate_active_profile();
            }
            self.draw_profile_selector_panel(ui);
            ui.separator();
            section_header(ui, "Current game setup", "Launch this profile or prepare its game setup.");
            {
                let profile = &self.profile.items[profile_index];
                installation_action = self.draw_profile_installation_panel(ui, profile);
            }
            ui.separator();
            section_header(ui, &profile_name, "Profile and game setup details.");
            self.draw_profile_management_panel(ui, &profile_name);
            {
                let profile = &self.profile.items[profile_index];
                composition_action = self.draw_profile_composition_panel(ui, profile);
            }
            self.draw_profile_launch_panel(ui);
        } else {
            ui.columns(2, |columns| {
                section_header(&mut columns[0], "Saved profiles", "A profile remembers one game setup and its launch options.");
                if columns[0]
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_context_switch_edits(),
                        egui::Button::new("Make a copy of this profile"),
                    )
                    .on_disabled_hover_text("Discard the Create draft or save/discard workspace edits before duplicating a profile.")
                    .clicked()
                {
                    self.duplicate_active_profile();
                }
                self.draw_profile_selector_panel(&mut columns[0]);

                section_header(&mut columns[1], "Current game setup", "Launch this profile or prepare its game setup.");
                {
                    let profile = &self.profile.items[profile_index];
                    installation_action = self.draw_profile_installation_panel(&mut columns[1], profile);
                }
                columns[1].separator();
                section_header(&mut columns[1], &profile_name, "Profile and game setup details.");
                self.draw_profile_management_panel(&mut columns[1], &profile_name);
                {
                    let profile = &self.profile.items[profile_index];
                    composition_action = self.draw_profile_composition_panel(&mut columns[1], profile);
                }
                self.draw_profile_launch_panel(&mut columns[1]);
            });
        }
        if let Some(action) = composition_action {
            match action {
                ProfileCompositionAction::BrowseContent => {
                    self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
                }
                ProfileCompositionAction::CreateContent => {
                    self.activate_navigation_section(super::super::Section::Create);
                }
                ProfileCompositionAction::EditComposition(selector) => {
                    if self.authoring.pending_composition_role_change.is_some() {
                        self.status = "Confirm or keep the pending game engine or game choice before opening another project.".to_owned();
                    } else {
                        self.authoring.composition_edit_selector = selector;
                        self.authoring.composition_edit = None;
                        self.authoring.composition_original_links = None;
                        self.activate_navigation_section(super::super::Section::Compose);
                        self.load_source_composition();
                    }
                }
                ProfileCompositionAction::PreviewPlan => self.preview_selected_composition(),
                ProfileCompositionAction::DownloadDependencies(ids) => {
                    self.download_profile_composition_dependencies(ids);
                }
            }
        }
        if let Some(action) = installation_action {
            match action {
                ProfileInstallationAction::Play => self.run_installed_selected(),
                ProfileInstallationAction::Repair => {
                    self.repair_selected_installation();
                }
                ProfileInstallationAction::Verify => {
                    self.verify_selected_installation();
                }
                ProfileInstallationAction::Update => {
                    self.update_selected();
                }
                ProfileInstallationAction::ReviewPlan => {
                    self.preview_selected_composition();
                }
                ProfileInstallationAction::BuildAndPlay => self.run_selected(),
                ProfileInstallationAction::Install => {
                    self.install_selected();
                }
                ProfileInstallationAction::ValidateSource => self.validate_selected_source(),
                ProfileInstallationAction::VerifyInstalled => {
                    self.verify_selected_installation();
                }
                ProfileInstallationAction::AdoptInstalledGraph => {
                    self.adopt_installed_profile_graph();
                }
                ProfileInstallationAction::DiscoverContent => {
                    self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
                    self.discover();
                }
            }
        }
    }
}
