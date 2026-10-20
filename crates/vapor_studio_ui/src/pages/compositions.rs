use super::super::StudioApp;
use super::super::components::design::{
    contextual_split, page_header, section_header, use_stacked_panels,
};
use super::compositions_content_panel::CompositionContentAction;
use super::compositions_installation_panel::CompositionInstallationAction;
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_compositions(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            "Compositions",
            "Your compositions",
            "Each composition is a complete playable setup. Choose one to install, launch, or customize.",
        );
        let composition_index = self.composition.active_index;
        let Some(composition_name) = self
            .composition
            .items
            .get(composition_index)
            .map(|composition| composition.name.clone())
        else {
            return;
        };
        let mut composition_action = None;
        let mut installation_action = None;
        if use_stacked_panels(ui) {
            section_header(
                ui,
                "Your compositions",
                "Each composition has its own content and launch settings.",
            );
            if ui
                .add_enabled(
                    !self.operation.running && !self.has_unsaved_context_switch_edits(),
                    egui::Button::new("Make a copy of this composition"),
                )
                .on_disabled_hover_text(
                    "Discard the Create draft or save/discard workspace edits before duplicating a composition.",
                )
                .clicked()
            {
                self.duplicate_active_composition();
            }
            self.draw_composition_selector_panel(ui);
            ui.separator();
            section_header(
                ui,
                "Selected composition",
                "Install or launch this composition.",
            );
            {
                let composition = &self.composition.items[composition_index];
                installation_action = self.draw_composition_installation_panel(ui, composition);
            }
            ui.separator();
            section_header(ui, &composition_name, "Composition details.");
            self.draw_composition_management_panel(ui, &composition_name);
            {
                let composition = &self.composition.items[composition_index];
                composition_action = self.draw_composition_content_panel(ui, composition);
            }
            self.draw_composition_launch_panel(ui);
        } else {
            contextual_split(ui, |compositions, details| {
                section_header(
                    compositions,
                    "Your compositions",
                    "Choose a composition to play or customize.",
                );
                if compositions
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_context_switch_edits(),
                        egui::Button::new("Make a copy of this composition"),
                    )
                    .on_disabled_hover_text("Discard the Create draft or save/discard workspace edits before duplicating a composition.")
                    .clicked()
                {
                    self.duplicate_active_composition();
                }
                self.draw_composition_selector_panel(compositions);

                section_header(
                    details,
                    &composition_name,
                    "Playable content and launch settings.",
                );
                {
                    let composition = &self.composition.items[composition_index];
                    installation_action =
                        self.draw_composition_installation_panel(details, composition);
                }
                details.separator();
                self.draw_composition_management_panel(details, &composition_name);
                {
                    let composition = &self.composition.items[composition_index];
                    composition_action = self.draw_composition_content_panel(details, composition);
                }
                self.draw_composition_launch_panel(details);
            });
        }
        if let Some(action) = composition_action {
            match action {
                CompositionContentAction::BrowseContent => {
                    self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
                }
                CompositionContentAction::CreateContent => {
                    self.activate_navigation_section(super::super::Section::Create);
                }
                CompositionContentAction::EditComposition(selector) => {
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
                CompositionContentAction::PreviewPlan => self.preview_selected_composition(),
                CompositionContentAction::DownloadDependencies(ids) => {
                    self.download_composition_dependencies(ids);
                }
            }
        }
        if let Some(action) = installation_action {
            match action {
                CompositionInstallationAction::Play => self.run_installed_selected(),
                CompositionInstallationAction::Repair => {
                    self.repair_selected_installation();
                }
                CompositionInstallationAction::Verify => {
                    self.verify_selected_installation();
                }
                CompositionInstallationAction::Update => {
                    self.update_selected();
                }
                CompositionInstallationAction::ReviewPlan => {
                    self.preview_selected_composition();
                }
                CompositionInstallationAction::BuildAndPlay => self.run_selected(),
                CompositionInstallationAction::OpenProject => {
                    if let Some(path) = self.composition.items[composition_index]
                        .workspace
                        .as_ref()
                        .map(|workspace| workspace.root.clone())
                    {
                        self.open_workspace_path_to(path, super::super::Section::Compositions);
                    }
                }
                CompositionInstallationAction::Install => {
                    self.install_selected();
                }
                CompositionInstallationAction::ValidateSource => self.validate_selected_source(),
                CompositionInstallationAction::VerifyInstalled => {
                    self.verify_selected_installation();
                }
                CompositionInstallationAction::AdoptInstalledGraph => {
                    self.adopt_installed_composition_graph();
                }
                CompositionInstallationAction::DiscoverContent => {
                    self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
                    self.discover();
                }
            }
        }
    }
}
