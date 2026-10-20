use super::super::StudioApp;
use super::super::components::design::{
    StatusTone, discard_confirmation, status_text, use_stacked_form,
};
use super::super::source_authoring::PendingManifestDraftDiscard;
use eframe::egui;

#[derive(Clone, Copy)]
enum WorkshopMetadataAction {
    Save,
    Discard,
    Close,
}

#[derive(Clone, Copy)]
enum RuntimeOutputsAction {
    Save,
    Discard,
    Close,
}

impl StudioApp {
    pub(super) fn draw_workspace_authoring_panel(&mut self, ui: &mut egui::Ui) {
        let operation_running = self.operation.running;
        let discard_pending = self.authoring.pending_manifest_discard.is_some();
        let mut workshop_action = None;
        if let Some(metadata) = self.authoring.workshop_metadata_edit.as_ref() {
            ui.add_enabled_ui(!operation_running && !discard_pending, |ui| {
                ui.group(|ui| {
                ui.heading(format!("Workshop metadata · {}", metadata.id));
                ui.label("Edit the details for a future Workshop upload. Saving here does not publish or upload anything.");
                let mut changed = false;
                ui.horizontal_wrapped(|ui| {
                    ui.label("Steam app");
                    changed |= ui.text_edit_singleline(&mut self.authoring.workshop_metadata_app_id).changed();
                    ui.label("Workshop item ID");
                    changed |= ui.text_edit_singleline(&mut self.authoring.workshop_metadata_published_file_id).changed();
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Visibility");
                    egui::ComboBox::from_id_salt("edit-source-workshop-visibility")
                        .selected_text(if self.authoring.workshop_metadata_visibility.is_empty() {
                            "Not set"
                        } else {
                            &self.authoring.workshop_metadata_visibility
                        })
                        .show_ui(ui, |ui| {
                            changed |= ui.selectable_value(&mut self.authoring.workshop_metadata_visibility, String::new(), "Not set").changed();
                            for visibility in ["private", "friends-only", "public"] {
                                changed |= ui.selectable_value(
                                    &mut self.authoring.workshop_metadata_visibility,
                                    visibility.to_owned(),
                                    visibility,
                                ).changed();
                            }
                        });
                    ui.label("Title");
                    changed |= ui.text_edit_singleline(&mut self.authoring.workshop_metadata_title).changed();
                });
                ui.label("Description");
                changed |= ui.add(
                    egui::TextEdit::multiline(&mut self.authoring.workshop_metadata_description)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                ).changed();
                ui.horizontal_wrapped(|ui| {
                    ui.label("Tags · comma separated");
                    changed |= ui.text_edit_singleline(&mut self.authoring.workshop_metadata_tags).changed();
                    ui.label("Change note");
                    changed |= ui.text_edit_singleline(&mut self.authoring.workshop_metadata_change_note).changed();
                });
                self.authoring.manifest_metadata_dirty |= changed;
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(
                        !operation_running
                            && !discard_pending
                            && !self.workspace.editor.has_dirty_documents()
                            && self.authoring.manifest_metadata_dirty,
                        egui::Button::new("Save Workshop metadata"),
                    ).clicked() {
                        workshop_action = Some(WorkshopMetadataAction::Save);
                    }
                    if ui.add_enabled(
                        !operation_running && !discard_pending && self.authoring.manifest_metadata_dirty,
                        egui::Button::new("Discard draft"),
                    ).clicked() {
                        workshop_action = Some(WorkshopMetadataAction::Discard);
                    }
                    if ui.add_enabled(
                        !operation_running && !self.authoring.manifest_metadata_dirty && !discard_pending,
                        egui::Button::new("Close"),
                    ).clicked() {
                        workshop_action = Some(WorkshopMetadataAction::Close);
                    }
                    if self.authoring.manifest_metadata_dirty {
                        status_text(ui, "Unsaved metadata draft", StatusTone::Caution);
                    }
                });
                });
            });
        }
        match workshop_action {
            Some(WorkshopMetadataAction::Save) => self.save_source_workshop_metadata(),
            Some(WorkshopMetadataAction::Discard) => {
                self.authoring.pending_manifest_discard =
                    Some(PendingManifestDraftDiscard::WorkshopMetadata);
            }
            Some(WorkshopMetadataAction::Close) => {
                self.authoring.workshop_metadata_edit = None;
            }
            None => {}
        }

        let mut outputs_action = None;
        if let Some(outputs) = self.authoring.runtime_outputs_edit.as_ref() {
            ui.add_enabled_ui(!operation_running && !discard_pending, |ui| {
                ui.group(|ui| {
                ui.heading(format!("Runtime outputs · {}", outputs.id));
                ui.label("Declare Cargo build outputs copied into the deployed artifact's bin/ and lib/ directories. Enter one file name per line; paths are not accepted.");
                if use_stacked_form(ui) {
                    ui.label("Executables");
                    let binaries_changed = ui
                        .add(
                            egui::TextEdit::multiline(
                                &mut self.authoring.runtime_binaries_text,
                            )
                            .desired_rows(4)
                            .desired_width(f32::INFINITY)
                            .hint_text("spacetime-engine"),
                        )
                        .changed();
                    ui.label("Libraries");
                    let libraries_changed = ui
                        .add(
                            egui::TextEdit::multiline(
                                &mut self.authoring.runtime_libraries_text,
                            )
                            .desired_rows(4)
                            .desired_width(f32::INFINITY)
                            .hint_text("libexample.so"),
                        )
                        .changed();
                    self.authoring.manifest_metadata_dirty |= binaries_changed || libraries_changed;
                } else {
                    ui.columns(2, |columns| {
                        columns[0].label("Executables");
                        let binaries_changed = columns[0]
                            .add(
                                egui::TextEdit::multiline(
                                    &mut self.authoring.runtime_binaries_text,
                                )
                                .desired_rows(4)
                                .desired_width(f32::INFINITY)
                                .hint_text("spacetime-engine"),
                            )
                            .changed();
                        columns[1].label("Libraries");
                        let libraries_changed = columns[1]
                            .add(
                                egui::TextEdit::multiline(
                                    &mut self.authoring.runtime_libraries_text,
                                )
                                .desired_rows(4)
                                .desired_width(f32::INFINITY)
                                .hint_text("libexample.so"),
                            )
                            .changed();
                        self.authoring.manifest_metadata_dirty |=
                            binaries_changed || libraries_changed;
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(
                        !operation_running
                            && !discard_pending
                            && !self.workspace.editor.has_dirty_documents()
                            && self.authoring.manifest_metadata_dirty,
                        egui::Button::new("Save runtime outputs"),
                    ).clicked() {
                        outputs_action = Some(RuntimeOutputsAction::Save);
                    }
                    if ui.add_enabled(
                        !operation_running && !discard_pending && self.authoring.manifest_metadata_dirty,
                        egui::Button::new("Discard draft"),
                    ).clicked() {
                        outputs_action = Some(RuntimeOutputsAction::Discard);
                    }
                    if ui.add_enabled(
                        !operation_running && !self.authoring.manifest_metadata_dirty && !discard_pending,
                        egui::Button::new("Close"),
                    ).clicked() {
                        outputs_action = Some(RuntimeOutputsAction::Close);
                    }
                    if self.authoring.manifest_metadata_dirty {
                        status_text(ui, "Unsaved runtime output changes", StatusTone::Caution);
                    }
                });
                });
            });
        }
        match outputs_action {
            Some(RuntimeOutputsAction::Save) => self.save_source_runtime_outputs(),
            Some(RuntimeOutputsAction::Discard) => {
                self.authoring.pending_manifest_discard =
                    Some(PendingManifestDraftDiscard::RuntimeOutputs);
            }
            Some(RuntimeOutputsAction::Close) => self.authoring.runtime_outputs_edit = None,
            None => {}
        }

        if let Some(pending) = self.authoring.pending_manifest_discard {
            let (title, description) = match pending {
                PendingManifestDraftDiscard::WorkshopMetadata => (
                    "Discard unsaved Workshop metadata?",
                    "The fields will return to the values from the saved source manifest.",
                ),
                PendingManifestDraftDiscard::RuntimeOutputs => (
                    "Discard unsaved runtime-output changes?",
                    "The executable and library lists will return to the saved source manifest.",
                ),
            };
            let mut decision = None;
            ui.add_enabled_ui(!operation_running, |ui| {
                decision = discard_confirmation(ui, title, description);
            });
            if let Some(discard) = decision {
                self.authoring.pending_manifest_discard = None;
                if discard {
                    match pending {
                        PendingManifestDraftDiscard::WorkshopMetadata => {
                            if let Some(policy) = self
                                .authoring
                                .workshop_metadata_edit
                                .as_ref()
                                .map(|metadata| metadata.policy.clone())
                            {
                                self.set_workshop_metadata_fields(&policy);
                                self.authoring.manifest_metadata_dirty = false;
                            }
                        }
                        PendingManifestDraftDiscard::RuntimeOutputs => {
                            if let Some(outputs) = self.authoring.runtime_outputs_edit.clone() {
                                self.set_runtime_output_fields(&outputs);
                                self.authoring.manifest_metadata_dirty = false;
                            }
                        }
                    }
                }
            }
        }
    }
}
