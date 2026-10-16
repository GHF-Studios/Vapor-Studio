use super::super::{Section, StudioApp, source_content_selector};
use eframe::egui;

impl StudioApp {
    pub(in crate::app) fn draw_publish(&mut self, ui: &mut egui::Ui) {
        ui.heading("Publish");
        ui.label("Review the exact destination and payload before an external upload.");

        ui.group(|ui| {
            ui.heading("Steam Workshop content");
            ui.label("Stage an authored item from Discover. The staged review remains available here while you inspect its identity, files, dependencies, and target.");
            let Some(preview) = self.workshop.preview.clone() else {
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
                    self.publish_reviewed_workshop_content(
                        identity,
                        preview.clone(),
                    );
                }
                if ui
                    .add_enabled(!self.operation.running, egui::Button::new("Discard review"))
                    .clicked()
                {
                    self.invalidate_workshop_review();
                }
            });
        });

        ui.group(|ui| {
            ui.heading("Vapor app on Steam");
            ui.label("Root packaging produces a read-only plan for the Vapor Steam app and depot. The GUI does not publish a root build yet.");
            ui.label(format!(
                "AppID {} · DepotID {} · branch vapor-dev",
                vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                vapor_sdk_core::root::types::ROOT_STEAM_DEPOT_ID,
            ));
            if ui
                .add_enabled(
                    !self.operation.running,
                    egui::Button::new("Preview root package"),
                )
                .clicked()
            {
                self.preview_steam_package();
            }
        });
    }
}
