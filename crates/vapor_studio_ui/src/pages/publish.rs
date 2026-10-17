use super::super::root_publishing::{PublishTab, root_publish_confirmation};
use super::super::components::design::page_header;
use super::super::{Section, StudioApp, source_content_selector};
use eframe::egui;
use vapor_sdk_core::root::types::{ROOT_STEAM_APP_ID, ROOT_STEAM_DEPOT_ID};

enum WorkshopPublishAction {
    Publish(String, vapor_studio_core::WorkshopPackagePreview),
    DiscardReview,
}

impl StudioApp {
    pub(in crate::app) fn draw_publish(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Share", "Publish", "Review the destination and exact payload before sending anything outside this device.");

        ui.horizontal(|ui| {
            if ui.add(egui::Button::selectable(self.root_publishing.tab == PublishTab::WorkshopContent, "Workshop content")).clicked() {
                self.root_publishing.tab = PublishTab::WorkshopContent;
            }
            if ui.add(egui::Button::selectable(self.root_publishing.tab == PublishTab::VaporApp, "Vapor app release")).clicked() {
                self.root_publishing.tab = PublishTab::VaporApp;
            }
        });

        if self.root_publishing.tab == PublishTab::WorkshopContent {
        let mut action = None;
        ui.group(|ui| {
            ui.heading("Publish a Workshop item");
            ui.label("Stage an authored item from Discover. The staged review remains available here while you inspect its identity, files, dependencies, and target.");
            let Some(preview) = self.workshop.preview.as_ref() else {
                ui.weak("No Workshop artifact is staged for review.");
                if ui
                    .add_enabled(
                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                        egui::Button::new("Find content to stage"),
                    )
                    .clicked()
                {
                    self.section = Section::Discover;
                    self.discover();
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
                .any(|content| source_content_selector(content) == identity);
            let conflicted = self.catalog.items.iter().any(|content| {
                content.identity().id == preview.id
                    && content.identity().version == preview.version
                    && !content.is_unambiguous()
            });

            ui.separator();
            ui.strong(&identity);
            ui.label(format!(
                "Title: {} · AppID: {} · visibility: {}",
                preview.title.as_deref().unwrap_or(&preview.id),
                preview.app_id.map_or_else(|| "missing".to_owned(), |id| id.to_string()),
                preview.visibility.as_deref().unwrap_or("not set"),
            ));
            ui.label(format!(
                "PublishedFileId: {} · target: {}",
                preview.published_file_id.as_deref().unwrap_or("new item"),
                preview.target,
            ));
            ui.label(format!(
                "{} files · {} bytes · payload {}",
                preview.files,
                preview.bytes,
                preview.root.display(),
            ));
            ui.small(format!("Fingerprint: {}", preview.fingerprint));

            ui.collapsing("Staged files", |ui| {
                for path in preview.file_paths.iter().take(50) {
                    ui.small(path);
                }
                if preview.file_paths.len() > 50 {
                    ui.weak(format!("{} more files", preview.file_paths.len() - 50));
                }
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
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "The authored source is no longer available in the active workspace. Restage it from Discover.",
                );
            }
            if conflicted {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Local records for this exact version disagree. Resolve the catalog conflict before publishing.",
                );
            }

            let publish_phrase = format!("PUBLISH {}", preview.id);
            ui.separator();
            ui.strong(if preview.published_file_id.is_some() {
                "Publish Workshop update"
            } else {
                "Create and publish Workshop item"
            });
            ui.label(format!(
                "Steam account · {}",
                if self.workshop.publisher_account.trim().is_empty() {
                    "enter the publisher account"
                } else {
                    self.workshop.publisher_account.trim()
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
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_workspace_edits()
                            && local_source
                            && !conflicted
                            && !self.workshop.publisher_account.trim().is_empty()
                            && self.workshop.confirmation.trim() == publish_phrase,
                        egui::Button::new(if preview.published_file_id.is_some() {
                            "Publish update"
                        } else {
                            "Create Workshop item"
                        }),
                    )
                    .clicked()
                {
                    action = Some(WorkshopPublishAction::Publish(identity.clone(), preview.clone()));
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
                self.publish_reviewed_workshop_content(identity, preview);
            }
            Some(WorkshopPublishAction::DiscardReview) => self.invalidate_workshop_review(),
            None => {}
        }
        } else {
        ui.group(|ui| {
            ui.heading("Publish a Vapor app build");
            ui.label("SteamPipe uploads a Vapor application release to the Steam app and depot shown below.");
            ui.label(format!(
                "Steam AppID {ROOT_STEAM_APP_ID} · DepotID {ROOT_STEAM_DEPOT_ID}",
            ));

            let mut settings_changed = false;
            let field_width = (ui.available_width() - 160.0).clamp(180.0, 420.0);
            egui::Grid::new("root-publish-settings")
                .num_columns(2)
                .spacing([14.0, 9.0])
                .show(ui, |ui| {
                    ui.label("Publisher account");
                    settings_changed |= ui.add_enabled(
                        !self.operation.running,
                        egui::TextEdit::singleline(&mut self.root_publishing.account)
                            .desired_width(field_width)
                            .hint_text("Steamworks account name"),
                    ).changed();
                    ui.end_row();
                    ui.label("Build description");
                    settings_changed |= ui.add_enabled(
                        !self.operation.running,
                        egui::TextEdit::singleline(&mut self.root_publishing.description)
                            .desired_width(field_width)
                            .hint_text("Vapor root package"),
                    ).changed();
                    ui.end_row();
                    ui.label("Beta branch");
                    settings_changed |= ui.add_enabled(
                        !self.operation.running,
                        egui::TextEdit::singleline(&mut self.root_publishing.set_live_branch)
                            .desired_width(field_width)
                            .hint_text("Leave empty to upload without changing a branch"),
                    ).changed();
                    ui.end_row();
                });
            ui.weak("SteamCMD handles password and Steam Guard prompts locally. Leaving the branch empty keeps this upload from changing what players receive.");
            if settings_changed {
                self.invalidate_root_publish_review();
            }

            ui.horizontal(|ui| {
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
                ui.strong(if preview.planned { "Reviewed SteamPipe upload" } else { "SteamPipe result" });
                ui.label(format!(
                    "App {} · depot {} · {} files · {} source roots",
                    preview.package.app_id,
                    preview.package.depot_id,
                    preview.package.copied_files,
                    preview.package.included_roots.len(),
                ));
                ui.label(format!("Account: {} · branch: {}", preview.account, preview.set_live.as_deref().unwrap_or("unchanged")));
                ui.small(format!("SteamCMD: {}", preview.steamcmd.display()));
                ui.small(format!("Package output: {}", preview.package.package_root.display()));
                ui.collapsing("Included depot paths", |ui| {
                    for path in &preview.package.included_roots {
                        ui.small(path.display().to_string());
                    }
                });
                if preview.planned {
                    let confirmation = root_publish_confirmation(preview.set_live.as_deref());
                    ui.colored_label(egui::Color32::YELLOW, "This sends the reviewed build to Steamworks. SteamCMD may prompt for your password or Steam Guard code.");
                    if preview.set_live.is_some() {
                        ui.colored_label(egui::Color32::YELLOW, "The selected beta branch will be changed after upload.");
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
                    if status.success() {
                        ui.colored_label(egui::Color32::GREEN, "SteamCMD reported a successful upload.");
                    } else {
                        ui.colored_label(egui::Color32::RED, format!("SteamCMD exited with {status}."));
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
