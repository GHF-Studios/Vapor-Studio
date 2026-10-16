use super::super::StudioApp;
use eframe::egui;
use std::path::{Path, PathBuf};
use vapor_core::DevelopmentOperation;

impl StudioApp {
    pub(in crate::app) fn draw_workspace(&mut self, ui: &mut egui::Ui) {
        ui.heading("Workspace");
        ui.label("Author content and maintain the selected Vapor workspace.");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.workspace.files_open, false, "Overview");
            let has_workspace = !self.workspace.root.trim().is_empty();
            if ui
                .add_enabled(
                    has_workspace,
                    egui::Button::selectable(self.workspace.files_open, "Files"),
                )
                .clicked()
            {
                self.workspace.files_open = true;
            }
        });
        ui.separator();
        if !self.workspace.files_open {
            ui.label("Operations use the workspace pin and Vapor-managed toolchain and cache.");
            let has_workspace = !self.workspace.root.trim().is_empty();
            ui.horizontal(|ui| {
                if ui.button("Refresh registered workspaces").clicked() {
                    self.refresh_registered_sources();
                }
                if self.workspace.registered_sources.is_empty() {
                    ui.weak("No workspaces loaded from this Vapor installation yet.");
                }
            });
            if !self.workspace.registered_sources.is_empty() {
                ui.label("Registered workspaces");
                for source in self.workspace.registered_sources.clone() {
                    let active = self.workspace.active_source.as_ref() == Some(&source);
                    let selected = Path::new(self.workspace.root.trim()) == source;
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !self.operation.running,
                                egui::Button::selectable(selected, source.display().to_string()),
                            )
                            .clicked()
                        {
                            self.open_workspace_path(source.clone());
                        }
                        if active {
                            ui.weak("Vapor active source");
                        }
                    });
                }
            }
            if !self.workspace.root.trim().is_empty() {
                let current = Path::new(self.workspace.root.trim());
                let already_registered = self.workspace.registered_sources.iter().any(|source| source == current);
                if !already_registered {
                    ui.horizontal(|ui| {
                        ui.label("This workspace is open for the current profile only.");
                        if ui.add_enabled(!self.operation.running, egui::Button::new("Add to Vapor sources")).clicked() {
                            self.remember_open_workspace();
                        }
                    });
                }
            }
            ui.horizontal(|ui| {
                ui.label("Workspace");
                ui.add_enabled(
                    !self.operation.running,
                    egui::TextEdit::singleline(&mut self.workspace.root_draft)
                        .desired_width(360.0)
                        .hint_text("Path to a Vapor workspace"),
                );
                if ui
                    .add_enabled(!self.operation.running, egui::Button::new("Open workspace"))
                    .clicked()
                {
                    self.open_workspace_path(PathBuf::from(self.workspace.root_draft.trim()));
                }
            });
            if !self.catalog.sources.is_empty() {
                ui.collapsing(format!("Content projects · {}", self.catalog.sources.len()), |ui| {
                    ui.label("These are the content projects Vapor discovered from this Workspace's explicit project registry.");
                    for content in self.catalog.sources.clone() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.strong(&content.id);
                                ui.weak(&content.kind);
                                ui.weak(content.version.as_deref().unwrap_or("workspace version"));
                            });
                            if !content.binaries.is_empty() || !content.libraries.is_empty() {
                                let mut outputs = Vec::new();
                                if !content.binaries.is_empty() {
                                    outputs.push(format!("binaries: {}", content.binaries.join(", ")));
                                }
                                if !content.libraries.is_empty() {
                                    outputs.push(format!("libraries: {}", content.libraries.join(", ")));
                                }
                                ui.small(format!("Declared runtime outputs · {}", outputs.join(" · ")));
                            }
                            if !content.dependencies.is_empty() {
                                ui.small(format!("Dependencies: {}", content.dependencies.join(" · ")));
                            }
                            let workshop = &content.workshop;
                            if workshop.app_id.is_some()
                                || workshop.published_file_id.is_some()
                                || workshop.visibility.is_some()
                                || workshop.title.is_some()
                                || workshop.description.is_some()
                                || !workshop.tags.is_empty()
                                || workshop.change_note.is_some()
                            {
                                ui.collapsing("Workshop publication metadata", |ui| {
                                    if let Some(app_id) = workshop.app_id {
                                        ui.small(format!("Steam AppID: {app_id}"));
                                    } else {
                                        ui.colored_label(
                                            egui::Color32::YELLOW,
                                            "Steam AppID not configured",
                                        );
                                    }
                                    ui.small(format!(
                                        "Published item: {}",
                                        workshop.published_file_id.as_deref().unwrap_or("new item")
                                    ));
                                    if let Some(visibility) = &workshop.visibility {
                                        ui.small(format!("Visibility: {visibility}"));
                                    }
                                    if let Some(title) = &workshop.title {
                                        ui.small(format!("Workshop title: {title}"));
                                    }
                                    if let Some(description) = &workshop.description {
                                        ui.small(format!("Description: {description}"));
                                    }
                                    if !workshop.tags.is_empty() {
                                        ui.small(format!("Tags: {}", workshop.tags.join(", ")));
                                    }
                                    if let Some(change_note) = &workshop.change_note {
                                        ui.small(format!("Default change note: {change_note}"));
                                    }
                                    ui.weak("This is authored intent; staging performs the authoritative package and publication checks.");
                                });
                            } else {
                                ui.small("Workshop publication: not configured");
                            }
                            ui.horizontal(|ui| {
                                let relative_manifest = content
                                    .manifest
                                    .strip_prefix(Path::new(self.workspace.root.trim()))
                                    .ok()
                                    .map(PathBuf::from);
                                if ui
                                    .add_enabled(
                                        !self.operation.running && !self.workspace.editor_dirty && !self.authoring.manifest_metadata_dirty,
                                        egui::Button::new("Validate"),
                                    )
                                    .on_hover_text("Validate this content manifest and its local references through Vapor Shell.")
                                    .clicked()
                                {
                                    self.validate_source_project(&content);
                                }
                                if let Some(relative_manifest) = relative_manifest {
                                    if ui
                                        .add_enabled(
                                            !self.operation.running && !self.workspace.editor_dirty && !self.authoring.manifest_metadata_dirty,
                                            egui::Button::new("Edit manifest"),
                                        )
                                        .on_disabled_hover_text("Save or discard the current file before opening a project manifest.")
                                        .clicked()
                                    {
                                        self.open_workspace_file(relative_manifest);
                                    }
                                }
                                if ui
                                    .add_enabled(
                                        !self.operation.running && !self.workspace.editor_dirty && !self.authoring.manifest_metadata_dirty,
                                        egui::Button::new("Edit Workshop metadata"),
                                    )
                                    .on_disabled_hover_text("Save or reload the open file first; Workshop edits use a fresh Shell-validated manifest snapshot.")
                                    .clicked()
                                {
                                    self.load_source_workshop_metadata(&content);
                                }
                                if ui
                                    .add_enabled(
                                        !self.operation.running && !self.workspace.editor_dirty && !self.authoring.manifest_metadata_dirty,
                                        egui::Button::new("Edit runtime outputs"),
                                    )
                                    .on_disabled_hover_text("Save or discard open edits first; runtime output edits use a fresh Shell-validated manifest snapshot.")
                                    .clicked()
                                {
                                    self.load_source_runtime_outputs(&content);
                                }
                                if ui
                                        .add_enabled(!self.operation.running && !self.workspace.editor_dirty && !self.authoring.manifest_metadata_dirty, egui::Button::new("Edit dependencies"))
                                        .clicked()
                                {
                                    self.authoring.composition_edit_selector =
                                        super::super::source_content_selector(&content);
                                    self.authoring.composition_edit = None;
                                    self.authoring.composition_original_links = None;
                                    self.section = super::super::Section::Compose;
                                    self.load_source_composition();
                                }
                                if content.kind == vapor_core::ContentKind::Packagepack.as_str()
                                    && ui
                                        .add_enabled(!self.operation.running && !self.workspace.editor_dirty && !self.authoring.manifest_metadata_dirty, egui::Button::new("Use in active profile"))
                                        .clicked()
                                {
                                    self.assign_profile_root(
                                        vapor_studio_core::ContentIdentity {
                                            id: content.id.clone(),
                                            version: content.version.clone(),
                                        },
                                        Some(PathBuf::from(self.workspace.root.trim())),
                                    );
                                }
                                if ui
                                    .add_enabled(
                                        !self.operation.running && !self.workspace.editor_dirty && !self.authoring.manifest_metadata_dirty,
                                        egui::Button::new("Unregister"),
                                    )
                                    .on_hover_text("Remove this project from Vapor discovery without deleting its files or changing Cargo membership.")
                                    .clicked()
                                {
                                    self.workspace.pending_unregistration = Some(content.clone());
                                }
                            });
                        });
                    }
                });
            }
            if let Some(project) = self.workspace.pending_unregistration.clone() {
                ui.group(|ui| {
                    ui.heading(format!("Unregister {}?", project.id));
                    ui.label(format!(
                        "Vapor will stop discovering this {} at {}. The project files and Cargo workspace membership will remain unchanged; you can adopt it again later.",
                        project.kind,
                        project.root.display(),
                    ));
                    ui.horizontal(|ui| {
                        if ui.add_enabled(
                            !self.operation.running && !self.workspace.editor_dirty && !self.authoring.manifest_metadata_dirty,
                            egui::Button::new("Unregister from Vapor"),
                        ).clicked() {
                            self.unregister_content_project();
                        }
                        if ui.add_enabled(!self.operation.running, egui::Button::new("Keep registered")).clicked() {
                            self.workspace.pending_unregistration = None;
                        }
                    });
                });
            }
            if let Some(metadata) = self.authoring.workshop_metadata_edit.clone() {
                ui.group(|ui| {
                    ui.heading(format!("Workshop metadata · {}", metadata.id));
                    ui.label("Edit the publication intent saved in this role manifest. This action does not upload to Steam.");
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label("Steam AppID");
                        changed |= ui.text_edit_singleline(&mut self.authoring.workshop_metadata_app_id).changed();
                        ui.label("PublishedFileId");
                        changed |= ui.text_edit_singleline(&mut self.authoring.workshop_metadata_published_file_id).changed();
                    });
                    ui.horizontal(|ui| {
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
                    ui.horizontal(|ui| {
                        ui.label("Tags · comma separated");
                        changed |= ui.text_edit_singleline(&mut self.authoring.workshop_metadata_tags).changed();
                        ui.label("Change note");
                        changed |= ui.text_edit_singleline(&mut self.authoring.workshop_metadata_change_note).changed();
                    });
                    self.authoring.manifest_metadata_dirty |= changed;
                    ui.horizontal(|ui| {
                        if ui.add_enabled(
                            !self.operation.running && !self.workspace.editor_dirty && self.authoring.manifest_metadata_dirty,
                            egui::Button::new("Save Workshop metadata"),
                        ).clicked() {
                            self.save_source_workshop_metadata();
                        }
                        if ui.add_enabled(!self.operation.running && self.authoring.manifest_metadata_dirty, egui::Button::new("Discard draft")).clicked() {
                            self.set_workshop_metadata_fields(&metadata.policy);
                            self.authoring.manifest_metadata_dirty = false;
                        }
                        if ui.add_enabled(!self.operation.running && !self.authoring.manifest_metadata_dirty, egui::Button::new("Close")).clicked() {
                            self.authoring.workshop_metadata_edit = None;
                        }
                        if self.authoring.manifest_metadata_dirty {
                            ui.colored_label(egui::Color32::YELLOW, "Unsaved metadata draft");
                        }
                    });
                });
            }
            if let Some(outputs) = self.authoring.runtime_outputs_edit.clone() {
                ui.group(|ui| {
                    ui.heading(format!("Runtime outputs · {}", outputs.id));
                    ui.label("Declare Cargo build outputs copied into the deployed artifact's bin/ and lib/ directories. Enter one file name per line; paths are not accepted.");
                    ui.columns(2, |columns| {
                        columns[0].label("Executables");
                        let binaries_changed = columns[0].add(
                            egui::TextEdit::multiline(&mut self.authoring.runtime_binaries_text)
                                .desired_rows(4)
                                .desired_width(f32::INFINITY)
                                .hint_text("spacetime-engine"),
                        ).changed();
                        columns[1].label("Libraries");
                        let libraries_changed = columns[1].add(
                            egui::TextEdit::multiline(&mut self.authoring.runtime_libraries_text)
                                .desired_rows(4)
                                .desired_width(f32::INFINITY)
                                .hint_text("libexample.so"),
                        ).changed();
                        self.authoring.manifest_metadata_dirty |= binaries_changed || libraries_changed;
                    });
                    ui.horizontal(|ui| {
                        if ui.add_enabled(
                            !self.operation.running && !self.workspace.editor_dirty && self.authoring.manifest_metadata_dirty,
                            egui::Button::new("Save runtime outputs"),
                        ).clicked() {
                            self.save_source_runtime_outputs();
                        }
                        if ui.add_enabled(!self.operation.running && self.authoring.manifest_metadata_dirty, egui::Button::new("Discard draft")).clicked() {
                            self.set_runtime_output_fields(&outputs);
                            self.authoring.manifest_metadata_dirty = false;
                        }
                        if ui.add_enabled(!self.operation.running && !self.authoring.manifest_metadata_dirty, egui::Button::new("Close")).clicked() {
                            self.authoring.runtime_outputs_edit = None;
                        }
                        if self.authoring.manifest_metadata_dirty {
                            ui.colored_label(egui::Color32::YELLOW, "Unsaved runtime output changes");
                        }
                    });
                });
            }
            if !self.workspace.root.trim().is_empty() {
                ui.horizontal(|ui| {
                    ui.label("Existing unregistered content");
                    if ui
                        .add_enabled(!self.operation.running && !self.workspace.editor_dirty, egui::Button::new("Find projects"))
                        .on_hover_text("Scans explicit role manifests and lists valid candidates. Nothing is registered until you choose Adopt.")
                        .clicked()
                    {
                        self.scan_unregistered_content_projects();
                    }
                });
                for candidate in self.workspace.unregistered_projects.candidates.clone() {
                    ui.horizontal(|ui| {
                        ui.label(&candidate.id);
                        ui.weak(format!("{} · {}", candidate.kind, candidate.path.display()));
                        if ui
                            .add_enabled(!self.operation.running && !self.workspace.editor_dirty, egui::Button::new("Adopt"))
                            .clicked()
                        {
                            self.adopt_content_project(candidate.path.clone());
                        }
                    });
                }
                for diagnostic in &self.workspace.unregistered_projects.diagnostics {
                    ui.colored_label(egui::Color32::YELLOW, diagnostic);
                }
            }
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!self.operation.running && !self.workspace.editor_dirty && !self.workspace.root.trim().is_empty(), egui::Button::new("Validate content"))
                    .on_disabled_hover_text("Save or reload the open file first; validation reads workspace files from disk.")
                    .clicked()
                {
                    self.validate_workspace_source();
                }
                for (label, operation) in [
                    ("Format", DevelopmentOperation::Fmt),
                    ("Check", DevelopmentOperation::Check),
                    ("Test", DevelopmentOperation::Test),
                    ("Build", DevelopmentOperation::Build),
                    ("Clean", DevelopmentOperation::Clean),
                ] {
                    if ui
                        .add_enabled(
                            !self.operation.running && !self.workspace.editor_dirty && has_workspace,
                            egui::Button::new(label),
                        )
                        .on_disabled_hover_text(if has_workspace {
                            "Save or reload the open file first; workspace operations read files from disk."
                        } else {
                            "Open a Vapor Workspace before running workspace operations."
                        })
                        .clicked()
                    {
                        self.run_operation(operation);
                    }
                }
            });
            ui.separator();
        } else if self.workspace.root.trim().is_empty() {
            ui.weak("Open a Vapor Workspace from Overview before editing its files.");
        } else {
            self.draw_workspace_files_panel(ui);
        }
    }
}
