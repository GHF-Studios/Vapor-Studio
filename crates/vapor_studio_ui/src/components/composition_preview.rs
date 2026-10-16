use eframe::egui;
use vapor_studio_core::{CompositionPreview, CompositionPreviewSource};

/// Render the common, read-only provenance and blocker view used by Compose and Library.
pub(crate) fn draw_composition_preview(
    ui: &mut egui::Ui,
    preview: &CompositionPreview,
) -> Option<String> {
    let mut workshop_download = None;
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.heading("Install plan");
            if preview.ready() {
                ui.colored_label(egui::Color32::LIGHT_GREEN, "Ready");
            } else {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("{} blocker(s)", preview.blockers.len()),
                );
            }
        });
        ui.weak("Dependency-first resolution · no packages, downloads, or installed files were changed.");
        for node in &preview.nodes {
            let source = match (node.source, node.already_installed) {
                (CompositionPreviewSource::Workspace, true) => "Workspace source · already installed",
                (CompositionPreviewSource::Workspace, false) => "Workspace source",
                (CompositionPreviewSource::Cache, true) => "Vapor cache · already installed",
                (CompositionPreviewSource::Cache, false) => "Vapor cache",
                (CompositionPreviewSource::Installed, _) => "Already installed · no source to update",
                (CompositionPreviewSource::DownloadRequired, _) => "Workshop acquisition needed",
            };
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} · {}{}",
                    node.kind,
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
            if let Some(published_file_id) = node.published_file_id.as_ref().filter(|_| {
                node.source == CompositionPreviewSource::DownloadRequired
            }) && ui.button(format!("Download Workshop item {published_file_id}")).clicked() {
                workshop_download = Some(published_file_id.clone());
            }
        }
        for blocker in &preview.blockers {
            ui.colored_label(egui::Color32::LIGHT_RED, blocker);
        }
        for warning in &preview.warnings {
            ui.weak(warning);
        }
    });
    workshop_download
}
