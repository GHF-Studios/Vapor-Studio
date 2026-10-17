use super::super::StudioApp;
use super::super::components::design::{page_header, section_header};
use super::library_composition_panel::ProfileCompositionAction;
use super::library_installation_panel::{
    ProfileInstallationAction, ProfilePrimaryAction,
};
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_library(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Play", "Play profiles", "Choose a composition, prepare its installation, and manage how it launches.");
        let profile_index = self.profile.active_index;
        let Some(profile_name) = self.profile.items.get(profile_index).map(|profile| profile.name.clone()) else {
            return;
        };
        let mut composition_action = None;
        let mut installation_action = None;
        if ui.available_width() < 820.0 {
            section_header(ui, "Launch profiles", "Each profile keeps its own content selection and launch arguments.");
            if ui
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_context_switch_edits(),
                    egui::Button::new("Duplicate selected profile"),
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
            section_header(ui, &profile_name, "Selected composition and local installation state.");
            self.draw_profile_management_panel(ui, &profile_name);
            {
                let profile = &self.profile.items[profile_index];
                composition_action = self.draw_profile_composition_panel(ui, profile);
                installation_action = self.draw_profile_installation_panel(ui, profile);
            }
            self.draw_profile_launch_panel(ui);
        } else {
            ui.columns(2, |columns| {
                columns[0].set_min_width(300.0);
                section_header(&mut columns[0], "Launch profiles", "Each profile keeps its own content selection and launch arguments.");
                if columns[0]
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_context_switch_edits(),
                        egui::Button::new("Duplicate selected profile"),
                    )
                    .on_disabled_hover_text("Discard the Create draft or save/discard workspace edits before duplicating a profile.")
                    .clicked()
                {
                    self.duplicate_active_profile();
                }
                self.draw_profile_selector_panel(&mut columns[0]);

                columns[1].set_min_width(420.0);
                section_header(&mut columns[1], &profile_name, "Selected composition and local installation state.");
                self.draw_profile_management_panel(&mut columns[1], &profile_name);
                {
                    let profile = &self.profile.items[profile_index];
                    composition_action = self.draw_profile_composition_panel(&mut columns[1], profile);
                    installation_action = self.draw_profile_installation_panel(&mut columns[1], profile);
                }
                self.draw_profile_launch_panel(&mut columns[1]);
            });
        }
        if let Some(action) = composition_action {
            match action {
                ProfileCompositionAction::BrowseContent => {
                    self.section = super::super::Section::Discover;
                }
                ProfileCompositionAction::CreateContent => {
                    self.section = super::super::Section::Create;
                }
                ProfileCompositionAction::EditComposition(selector) => {
                    self.authoring.composition_edit_selector = selector;
                    self.authoring.composition_edit = None;
                    self.authoring.composition_original_links = None;
                    self.section = super::super::Section::Compose;
                    self.load_source_composition();
                }
                ProfileCompositionAction::PreviewPlan => self.preview_selected_composition(),
                ProfileCompositionAction::DownloadDependencies(ids) => {
                    self.download_profile_composition_dependencies(ids);
                }
            }
        }
        if let Some(action) = installation_action {
            match action {
                ProfileInstallationAction::Primary(ProfilePrimaryAction::PlayInstalled) => {
                    self.run_installed_selected();
                }
                ProfileInstallationAction::Primary(ProfilePrimaryAction::Repair) => {
                    self.repair_selected_installation();
                }
                ProfileInstallationAction::Primary(ProfilePrimaryAction::Verify) => {
                    self.verify_selected_installation();
                }
                ProfileInstallationAction::Primary(ProfilePrimaryAction::Update) => {
                    self.update_selected();
                }
                ProfileInstallationAction::Primary(ProfilePrimaryAction::ReviewPlan) => {
                    self.preview_selected_composition();
                }
                ProfileInstallationAction::Primary(ProfilePrimaryAction::BuildAndPlay) => {
                    self.run_selected();
                }
                ProfileInstallationAction::Primary(ProfilePrimaryAction::Install) => {
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
                    self.section = super::super::Section::Discover;
                    self.discover();
                }
            }
        }
    }
}
