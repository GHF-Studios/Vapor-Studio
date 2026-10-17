use super::design::{StatusTone, status_text};
use eframe::egui;
use vapor_studio_core::{CompositionPreview, CompositionPreviewSource};

/// Render the common, read-only provenance and blocker view used by Compose and Library.
pub(crate) fn draw_composition_preview(
    ui: &mut egui::Ui,
    preview: &CompositionPreview,
    can_import_missing_items: bool,
) -> Vec<String> {
    let mut workshop_downloads = Vec::new();
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.heading("Setup requirements");
            if preview.ready() {
                status_text(ui, "Ready", StatusTone::Positive);
            } else {
                status_text(
                    ui,
                    &format!("{} things to fix", preview.blockers.len()),
                    StatusTone::Caution,
                );
            }
        });
        ui.weak("This is a read-only plan. It has not downloaded or installed anything.");
        if !can_import_missing_items {
            status_text(
                ui,
                "Downloads are paused until Studio finishes its current task and your saved project files are up to date.",
                StatusTone::Caution,
            );
        }
        for node in &preview.nodes {
            let source = match (node.source, node.already_installed) {
                (CompositionPreviewSource::Workspace, true) => {
                    "Project folder copy · already installed"
                }
                (CompositionPreviewSource::Workspace, false) => "Project folder copy",
                (CompositionPreviewSource::Cache, true) => "Downloaded here · already ready",
                (CompositionPreviewSource::Cache, false) => "Downloaded here",
                (CompositionPreviewSource::Installed, _) => {
                    "Already installed · no source to update"
                }
                (CompositionPreviewSource::DownloadRequired, _) => "Download needed",
            };
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} · {}{}",
                    super::super::display_content_kind(&node.kind),
                    node.id,
                    node.version
                        .as_ref()
                        .map(|version| format!("@{version}"))
                        .unwrap_or_default()
                ));
                ui.weak(source);
            });
            for dependency in &node.dependencies {
                ui.small(format!(
                    "↳ {} · {}{}{}",
                    dependency.relationship,
                    dependency.id,
                    dependency
                        .version
                        .as_ref()
                        .map(|version| format!(" ({version})"))
                        .unwrap_or_default(),
                    if dependency.optional {
                        " · optional"
                    } else {
                        ""
                    },
                ));
            }
            if let Some(published_file_id) = node
                .published_file_id
                .as_ref()
                .filter(|_| node.source == CompositionPreviewSource::DownloadRequired)
                && !workshop_downloads.contains(published_file_id)
            {
                workshop_downloads.push(published_file_id.clone());
            }
        }
        if !workshop_downloads.is_empty() {
            ui.separator();
            ui.label(format!(
                "{} required Workshop item{} need to be downloaded before this setup can be installed.",
                workshop_downloads.len(),
                if workshop_downloads.len() == 1 { "" } else { "s" }
            ));
            let import_clicked = ui
                .add_enabled(
                    can_import_missing_items,
                    egui::Button::new(format!(
                        "Download {} missing item{}",
                        workshop_downloads.len(),
                        if workshop_downloads.len() == 1 { "" } else { "s" }
                    )),
                )
                .on_disabled_hover_text(
                    "Finish the current task and save or reload project edits before downloading these items.",
                )
                .clicked();
            if !import_clicked {
                workshop_downloads.clear();
            }
        }
        for blocker in &preview.blockers {
            status_text(ui, blocker, StatusTone::Error);
        }
        for warning in &preview.warnings {
            ui.weak(warning);
        }
    });
    workshop_downloads
}
