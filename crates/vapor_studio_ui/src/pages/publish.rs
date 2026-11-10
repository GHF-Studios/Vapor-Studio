use super::super::StudioApp;
use super::super::components::design::{StatusTone, page_header, status_text, use_stacked_form};
use super::super::application_release::{ApplicationReleaseState, PublishTab};
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
                    self.application_release.tab == PublishTab::WorkshopContent,
                    "Workshop content",
                ))
                .clicked()
            {
                self.application_release.tab = PublishTab::WorkshopContent;
            }
            if ui
                .add(egui::Button::selectable(
                    self.application_release.tab == PublishTab::SteamProduct,
                    "Loo-Cast Steam content",
                ))
                .clicked()
            {
                self.application_release.tab = PublishTab::SteamProduct;
            }
        });

        if self.application_release.tab == PublishTab::WorkshopContent {
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
                    "Local copies differ from the reviewed workspace source. Publishing uses the workspace source and does not change installed content.",
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
                ui.heading("Loo-Cast Steam content");
                ui.label("This Steam app is a content mount: the engine, game, packagepack, and composition manifest. Vapor itself—including its tools, caches, and user data—stays in Vapor home.");

                let mut settings_changed = false;
                if use_stacked_form(ui) {
                    ui.label("Publisher account");
                    settings_changed |= ui
                        .add_enabled(
                            !self.operation.running,
                            egui::TextEdit::singleline(&mut self.application_release.account)
                                .desired_width(ui.available_width())
                                .hint_text("Steamworks account name"),
                        )
                        .changed();
                    ui.label("Release description");
                    settings_changed |= ui
                        .add_enabled(
                            !self.operation.running,
                            egui::TextEdit::singleline(&mut self.application_release.description)
                                .desired_width(ui.available_width())
                                .hint_text("Loo-Cast content release"),
                        )
                        .changed();
                    ui.label("Beta branch");
                    settings_changed |= ui
                        .add_enabled(
                            !self.operation.running,
                            egui::TextEdit::singleline(&mut self.application_release.branch)
                                .desired_width(ui.available_width())
                                .hint_text("Leave empty to use the manifest default"),
                        )
                        .changed();
                } else {
                    let field_width = (ui.available_width() - 150.0).clamp(180.0, 420.0);
                    egui::Grid::new("loo-cast-content-release-settings")
                        .num_columns(2)
                        .spacing([14.0, 9.0])
                        .show(ui, |ui| {
                            ui.label("Publisher account");
                            settings_changed |= ui
                                .add_enabled(
                                    !self.operation.running,
                                    egui::TextEdit::singleline(
                                        &mut self.application_release.account,
                                    )
                                    .desired_width(field_width)
                                    .hint_text("Steamworks account name"),
                                )
                                .changed();
                            ui.end_row();
                            ui.label("Release description");
                            settings_changed |= ui
                                .add_enabled(
                                    !self.operation.running,
                                    egui::TextEdit::singleline(
                                        &mut self.application_release.description,
                                    )
                                    .desired_width(field_width)
                                    .hint_text("Loo-Cast content release"),
                                )
                                .changed();
                            ui.end_row();
                            ui.label("Beta branch");
                            settings_changed |= ui
                                .add_enabled(
                                    !self.operation.running,
                                    egui::TextEdit::singleline(&mut self.application_release.branch)
                                        .desired_width(field_width)
                                        .hint_text("Leave empty to use the manifest default"),
                                )
                                .changed();
                            ui.end_row();
                        });
                }
                ui.weak("Steam may request your password or Steam Guard code during upload. Publishing uses the exact files declared by the app manifest.");
                ui.label("Include ABI-specific product files for:");
                ui.horizontal(|ui| {
                    settings_changed |= ui
                        .add_enabled(
                            !self.operation.running,
                            egui::Checkbox::new(&mut self.application_release.linux, "Linux"),
                        )
                        .changed();
                    settings_changed |= ui
                        .add_enabled(
                            !self.operation.running,
                            egui::Checkbox::new(&mut self.application_release.windows, "Windows"),
                        )
                        .changed();
                });
                if self.application_release.runtime_targets().is_empty() {
                    status_text(
                        ui,
                        "Choose at least one ABI target before staging the Steam content release.",
                        StatusTone::Caution,
                    );
                }
                if settings_changed {
                    self.invalidate_application_release_review();
                }

                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !self.operation.running
                                && !self.application_release.account.trim().is_empty()
                                && !self.application_release.runtime_targets().is_empty(),
                            egui::Button::new("Review content release"),
                        )
                        .clicked()
                    {
                        self.stage_steam_product_release();
                    }
                    if self.application_release.preview.is_some()
                        && ui
                            .add_enabled(
                                !self.operation.running,
                                egui::Button::new("Discard review"),
                            )
                            .clicked()
                    {
                        self.invalidate_application_release_review();
                    }
                });

                if let Some(preview) = self.application_release.preview.as_ref() {
                    ui.separator();
                    ui.strong(if preview.planned {
                        "Reviewed content release"
                    } else {
                        "Content release result"
                    });
                    ui.label(format!(
                        "App {} · {} depot(s) · {} files",
                        preview.app_id,
                        preview.depot_ids.len(),
                        preview.files,
                    ));
                    ui.label(format!(
                        "Account: {} · branch: {}",
                        preview.account,
                        if preview.branch.is_empty() {
                            "unchanged"
                        } else {
                            &preview.branch
                        }
                    ));
                    ui.collapsing("ABI targets", |ui| {
                        for path in &preview.runtime_targets {
                            ui.small(path);
                        }
                    });
                    if preview.planned {
                        let confirmation = ApplicationReleaseState::confirmation_phrase(preview);
                        status_text(
                            ui,
                            "This uploads the reviewed Loo-Cast content mount. No Vapor runtime or user data is included unless the app manifest explicitly declares it.",
                            StatusTone::Caution,
                        );
                        ui.label(format!("Type {confirmation} to authorize this upload."));
                        ui.add_enabled(
                            !self.operation.running,
                            egui::TextEdit::singleline(
                                &mut self.application_release.confirmation,
                            )
                            .hint_text(&confirmation),
                        );
                        if ui
                            .add_enabled(
                                !self.operation.running
                                    && self.application_release.confirmation.trim()
                                        == confirmation,
                                egui::Button::new("Upload content release"),
                            )
                            .clicked()
                        {
                            self.publish_reviewed_steam_product();
                        }
                    } else if let Some(status) = preview.status.as_ref() {
                        if preview.succeeded {
                            status_text(ui, "Steam content upload completed.", StatusTone::Positive);
                        } else {
                            status_text(
                                ui,
                                &format!("Steam content upload failed ({status})."),
                                StatusTone::Error,
                            );
                        }
                    }
                }
                if !self.application_release.status.is_empty() {
                    ui.separator();
                    ui.label(&self.application_release.status);
                }
            });
        }
    }
}
