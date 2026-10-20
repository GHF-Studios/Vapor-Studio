use super::super::StudioApp;
use super::super::components::design::{StatusTone, page_header, status_text, use_stacked_form};
use super::super::root_publishing::{PublishTab, root_publish_confirmation};
use super::workspace_content_panel::{display_target, format_bytes};
use eframe::egui;

enum WorkshopPublishAction {
    Publish(
        String,
        Box<vapor_client_application::WorkshopPackagePreview>,
    ),
    DiscardReview,
}

impl StudioApp {
    pub(in crate::app) fn draw_publish(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            "Share",
            "Publish",
            "Choose where to share your content, then review exactly what will be sent before you publish.",
        );

        ui.horizontal_wrapped(|ui| {
            if ui
                .add(egui::Button::selectable(
                    self.root_publishing.tab == PublishTab::WorkshopContent,
                    "Workshop content",
                ))
                .clicked()
            {
                self.root_publishing.tab = PublishTab::WorkshopContent;
            }
            if ui
                .add(egui::Button::selectable(
                    self.root_publishing.tab == PublishTab::VaporApp,
                    "Vapor app release",
                ))
                .clicked()
            {
                self.root_publishing.tab = PublishTab::VaporApp;
            }
        });

        if self.root_publishing.tab == PublishTab::WorkshopContent {
            let mut action = None;
            ui.group(|ui| {
            ui.heading("Publish a Workshop item");
            ui.label("Prepare an upload from a project you own. Review its identity, included files, dependencies, and platforms here; nothing is sent until you confirm.");
            let Some(preview) = self.workshop.preview.as_ref() else {
                ui.weak("No Workshop upload is ready for review.");
                if ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new("Find a project to share"),
                    )
                    .clicked()
                {
                    if self.workspace.root.trim().is_empty() {
                        self.activate_navigation_section(super::super::Section::Workspace);
                        self.status =
                            "Open a project folder before preparing a Workshop upload.".to_owned();
                    } else {
                        self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
                        self.discover();
                    }
                }
                return;
            };

            let identity = preview
                .version
                .as_deref()
                .map(|version| format!("{}@{version}", preview.id))
                .unwrap_or_else(|| preview.id.clone());
            let local_source = self
                .catalog
                .sources
                .iter()
                .any(|content| content.id == preview.id && content.version == preview.version);
            let source_snapshot_current = self.catalog.source_snapshot_current();
            let catalog_snapshot_current = self.catalog.local_catalog_current();
            let conflicted = self.catalog.items.iter().any(|content| {
                content.id == preview.id
                    && content.version == preview.version
                    && !content.is_unambiguous()
            });

            ui.separator();
            ui.strong(&identity);
            ui.label(format!(
                "Title: {} · visibility: {}",
                preview.title.as_deref().unwrap_or(&preview.id),
                preview.visibility.as_deref().unwrap_or("not set"),
            ));
            ui.label(format!(
                "Available for {}",
                display_target(&preview.target),
            ));
            ui.label(format!("{} files · {}", preview.files, format_bytes(preview.bytes)));

            ui.collapsing("Files included", |ui| {
                for path in preview.file_paths.iter().take(50) {
                    ui.small(path);
                }
                if self.workshop.omitted_review_file_paths > 0 {
                    ui.weak(format!(
                        "Showing {} of {} included files.",
                        preview.file_paths.len(),
                        preview.files,
                    ));
                } else if preview.file_paths.len() > 50 {
                    ui.weak(format!("{} more files", preview.file_paths.len() - 50));
                }
            });
            ui.collapsing("Workshop details", |ui| {
                ui.label(format!(
                    "Steam app: {}",
                    preview.app_id.map_or_else(|| "not set".to_owned(), |id| id.to_string())
                ));
                if let Some(published_file_id) = preview.published_file_id.as_deref() {
                    ui.label(format!("Workshop item: {published_file_id}"));
                } else {
                    ui.label("This will create a new Workshop item.");
                }
                ui.label(format!("Build target: {}", preview.target));
            });
            if preview.dependencies.is_empty() {
                ui.weak("No declared dependencies.");
            } else {
                ui.label("Dependencies");
                for dependency in &preview.dependencies {
                    ui.small(format!("Requires {dependency}"));
                }
            }

            if !local_source {
                status_text(
                    ui,
                    "Open the project folder where this item is authored, then prepare a new review.",
                    StatusTone::Caution,
                );
            }
            if !source_snapshot_current {
                status_text(
                    ui,
                    "The project list could not refresh. Refresh it before publishing this staged version.",
                    StatusTone::Caution,
                );
            }
            if !catalog_snapshot_current {
                status_text(
                    ui,
                    "Studio could not confirm that this version is conflict-free. Refresh the on-device content list before publishing.",
                    StatusTone::Caution,
                );
            }
            if conflicted {
                status_text(
                    ui,
                    "On-device copies of this version disagree. Review the content details before publishing.",
                    StatusTone::Caution,
                );
            }

            let publisher_account = self.workshop.publisher_account.trim();
            let publish_phrase = format!("PUBLISH {} TO {}", preview.id, publisher_account);
            ui.separator();
            ui.strong(if preview.published_file_id.is_some() {
                "Publish Workshop update"
            } else {
                "Create and publish Workshop item"
            });
            ui.label(format!(
                "Steam account · {}",
                if publisher_account.is_empty() {
                    "enter the publisher account"
                } else {
                    publisher_account
                }
            ));
            ui.add_enabled(
                !self.operation.running,
                egui::TextEdit::singleline(&mut self.workshop.publisher_account)
                    .hint_text("Steam publisher account"),
            );
            ui.add_enabled(
                !self.operation.running,
                egui::TextEdit::singleline(&mut self.workshop.change_note)
                    .hint_text("Change note (optional)"),
            );
            ui.label(format!("Type {publish_phrase} to authorize this upload."));
            ui.add_enabled(
                !self.operation.running,
                egui::TextEdit::singleline(&mut self.workshop.confirmation)
                    .hint_text(&publish_phrase),
            );
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_workspace_edits()
                            && local_source
                            && source_snapshot_current
                            && catalog_snapshot_current
                            && !conflicted
                            && !self.workshop.publisher_account.trim().is_empty()
                            && self.workshop.confirmation.trim() == publish_phrase,
                        egui::Button::new(if preview.published_file_id.is_some() {
                            "Publish update"
                        } else {
                            "Create Workshop item"
                        }),
                    )
                    .on_disabled_hover_text(if !source_snapshot_current {
                        "Refresh the project list before publishing this staged version."
                    } else if !catalog_snapshot_current {
                        "Refresh the on-device content list before publishing; Studio cannot check for conflicts using out-of-date information."
                    } else if !local_source {
                        "Open the project folder where this item is authored, then prepare a new review."
                    } else if conflicted {
                        "Review and fix the conflicting on-device copies before publishing."
                    } else if self.has_unsaved_workspace_edits() {
                        "Save or discard workspace changes before publishing."
                    } else {
                        "Finish the current operation or complete the publishing confirmation."
                    })
                    .clicked()
                {
                    action = Some(WorkshopPublishAction::Publish(
                        identity.clone(),
                        Box::new(preview.clone()),
                    ));
                }
                if ui
                    .add_enabled(!self.operation.running, egui::Button::new("Discard review"))
                    .clicked()
                {
                    action = Some(WorkshopPublishAction::DiscardReview);
                }
            });
        });
            match action {
                Some(WorkshopPublishAction::Publish(identity, preview)) => {
                    self.publish_reviewed_workshop_content(identity, *preview);
                }
                Some(WorkshopPublishAction::DiscardReview) => self.invalidate_workshop_review(),
                None => {}
            }
        } else {
            ui.group(|ui| {
            ui.heading("Publish a Vapor app build");
            ui.label("This publishes a Vapor release build to the Steam app and depot selected for this installation.");
            ui.label(format!(
                "Steam app {} · depot {}",
                self.root_publishing.identity.app_id,
                self.root_publishing.identity.depot_id,
            ));

            let mut settings_changed = false;
            if use_stacked_form(ui) {
                ui.label("Publisher account");
                let field_width = ui.available_width();
                settings_changed |= ui
                    .add_enabled(
                        !self.operation.running,
                        egui::TextEdit::singleline(&mut self.root_publishing.account)
                            .desired_width(field_width)
                            .hint_text("Steamworks account name"),
                    )
                    .changed();
                ui.label("Build description");
                let field_width = ui.available_width();
                settings_changed |= ui
                    .add_enabled(
                        !self.operation.running,
                        egui::TextEdit::singleline(&mut self.root_publishing.description)
                            .desired_width(field_width)
                            .hint_text("Vapor root package"),
                    )
                    .changed();
                ui.label("Beta branch");
                let field_width = ui.available_width();
                settings_changed |= ui
                    .add_enabled(
                        !self.operation.running,
                        egui::TextEdit::singleline(&mut self.root_publishing.set_live_branch)
                            .desired_width(field_width)
                            .hint_text("Leave empty to upload without changing a branch"),
                    )
                    .changed();
            } else {
                let field_width = (ui.available_width() - 160.0).clamp(180.0, 420.0);
                egui::Grid::new("root-publish-settings")
                    .num_columns(2)
                    .spacing([14.0, 9.0])
                    .show(ui, |ui| {
                        ui.label("Publisher account");
                        settings_changed |= ui
                            .add_enabled(
                                !self.operation.running,
                                egui::TextEdit::singleline(&mut self.root_publishing.account)
                                    .desired_width(field_width)
                                    .hint_text("Steamworks account name"),
                            )
                            .changed();
                        ui.end_row();
                        ui.label("Build description");
                        settings_changed |= ui
                            .add_enabled(
                                !self.operation.running,
                                egui::TextEdit::singleline(
                                    &mut self.root_publishing.description,
                                )
                                .desired_width(field_width)
                                .hint_text("Vapor root package"),
                            )
                            .changed();
                        ui.end_row();
                        ui.label("Beta branch");
                        settings_changed |= ui
                            .add_enabled(
                                !self.operation.running,
                                egui::TextEdit::singleline(
                                    &mut self.root_publishing.set_live_branch,
                                )
                                .desired_width(field_width)
                                .hint_text("Leave empty to upload without changing a branch"),
                            )
                            .changed();
                        ui.end_row();
                    });
            }
            ui.weak("Steam may ask for your password or Steam Guard code during upload. Leave the branch empty to keep the current live branch unchanged.");
            if settings_changed {
                self.invalidate_root_publish_review();
            }

            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(
                    !self.operation.running && !self.root_publishing.account.trim().is_empty(),
                    egui::Button::new("Review Steam upload"),
                ).clicked() {
                    self.preview_steam_package();
                }
                if self.root_publishing.preview.is_some() && ui.add_enabled(
                    !self.operation.running,
                    egui::Button::new("Discard review"),
                ).clicked() {
                    self.invalidate_root_publish_review();
                }
            });

            if let Some(preview) = self.root_publishing.preview.as_ref() {
                ui.separator();
                ui.strong(if preview.planned { "Reviewed Steam upload" } else { "Steam upload result" });
                ui.label(format!(
                    "App {} · depot {} · {} files · {} source roots",
                    preview.app_id,
                    preview.depot_id,
                    preview.copied_files,
                    preview.included_roots.len(),
                ));
                ui.label(format!("Account: {} · branch: {}", preview.account, preview.set_live.as_deref().unwrap_or("unchanged")));
                ui.collapsing("Advanced package details", |ui| {
                    ui.small(format!("Upload command: {}", preview.steamcmd));
                    ui.small(format!("Package location: {}", preview.package_root));
                });
                ui.collapsing("Included depot paths", |ui| {
                    for path in &preview.included_roots {
                        ui.small(path);
                    }
                });
                if preview.planned {
                    let confirmation = root_publish_confirmation(preview.app_id, preview.set_live.as_deref());
                    status_text(ui, "This sends the reviewed build to Steam. Steam may ask for your password or Steam Guard code.", StatusTone::Caution);
                    if preview.set_live.is_some() {
                        status_text(ui, "The selected beta branch will be changed after upload.", StatusTone::Caution);
                    }
                    ui.label(format!("Type {confirmation} to authorize this upload."));
                    ui.add_enabled(
                        !self.operation.running,
                        egui::TextEdit::singleline(&mut self.root_publishing.confirmation).hint_text(&confirmation),
                    );
                    if ui.add_enabled(
                        !self.operation.running && self.root_publishing.confirmation.trim() == confirmation,
                        egui::Button::new(if preview.set_live.is_some() { "Upload and set branch live" } else { "Upload Steam build" }),
                    ).clicked() {
                        self.publish_steam_root();
                    }
                } else if let Some(status) = preview.status.as_ref() {
                    if preview.succeeded {
                        status_text(ui, "Steam upload completed.", StatusTone::Positive);
                    } else {
                        status_text(ui, &format!("Steam upload failed ({status})."), StatusTone::Error);
                    }
                }
            }
            if !self.root_publishing.status.is_empty() {
                ui.separator();
                ui.label(&self.root_publishing.status);
            }
        });
        }
    }
}
