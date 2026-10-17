use super::super::StudioApp;
use super::super::directory_picker::{DirectoryPicker, DirectoryPickerPurpose};
use eframe::egui;
use std::path::{Path, PathBuf};

impl StudioApp {
    pub(super) fn draw_workspace_registry_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("Choose the source workspace used by this play profile and authoring session.");
        let card_style = ui.style().clone();
        ui.columns(2, |columns| {
            egui::Frame::group(&card_style).show(&mut columns[0], |ui| {
                ui.heading("Current workspace");
                if self.workspace.root.trim().is_empty() {
                    ui.weak("No workspace is open for this profile.");
                } else {
                    ui.label(std::path::Path::new(self.workspace.root.trim())
                        .file_name().and_then(|name| name.to_str()).unwrap_or(&self.workspace.root));
                    ui.small(&self.workspace.root);
                    ui.weak("This workspace is the source context for Create, Compose, validation, and managed build/run operations.");
                }
                ui.add_space(8.0);
                ui.label("Open a Vapor workspace");
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled(
                        !self.operation.running,
                        egui::TextEdit::singleline(&mut self.workspace.root_draft)
                            .desired_width(360.0)
                            .hint_text("Path to a Vapor workspace"),
                    );
                    if ui
                        .add_enabled(!self.operation.running, egui::Button::new("Browse…"))
                        .clicked()
                    {
                        self.directory_picker = Some(DirectoryPicker::new(
                            DirectoryPickerPurpose::OpenWorkspace,
                            Path::new(self.workspace.root_draft.trim()),
                        ));
                    }
                    if ui
                        .add_enabled(
                            !self.operation.running && !self.workspace.root_draft.trim().is_empty(),
                            egui::Button::new("Open"),
                        )
                        .clicked()
                    {
                        self.open_workspace_path(PathBuf::from(self.workspace.root_draft.trim()));
                    }
                });
                if !self.workspace.root.trim().is_empty() {
                    let current = Path::new(self.workspace.root.trim());
                    let registered = self.workspace.registered_sources.iter().any(|source| source == current);
                    if registered {
                        ui.weak("This workspace is also registered as a Vapor source.");
                    } else {
                        ui.weak("The play profile remembers this workspace. Registering it as a Vapor source makes it available to other Vapor surfaces too.");
                        if ui.add_enabled(!self.operation.running, egui::Button::new("Register as Vapor source")).clicked() {
                            self.remember_open_workspace();
                        }
                    }
                }
            });
            egui::Frame::group(&card_style).show(&mut columns[1], |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Vapor sources");
                    if ui.add_enabled(!self.operation.running, egui::Button::new("Refresh")).clicked() {
                        self.refresh_registered_sources();
                    }
                });
                ui.label("Known workspaces registered with this Vapor installation.");
                if self.workspace.registered_sources.is_empty() {
                    ui.weak("No registered source workspaces loaded yet. Refresh to check this installation.");
                }
                for source in self.workspace.registered_sources.clone() {
                    let active = self.workspace.active_source.as_ref() == Some(&source);
                    let selected = Path::new(self.workspace.root.trim()) == source;
                    egui::Frame::group(&card_style).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.strong(source.file_name().and_then(|name| name.to_str()).unwrap_or("Workspace"));
                                ui.small(source.display().to_string());
                            });
                            if active {
                                ui.weak("Active source");
                            }
                            if ui
                                .add_enabled(
                                    !self.operation.running,
                                    egui::Button::selectable(selected, "Open"),
                                )
                                .clicked()
                            {
                                self.open_workspace_path(source.clone());
                            }
                        });
                    });
                }
            });
        });
    }
}
