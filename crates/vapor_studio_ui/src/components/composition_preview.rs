use super::design::{StatusTone, status_text};
use eframe::egui;
use vapor_client_application::{CompositionPreview, CompositionPreviewSource};

/// Render the common, read-only provenance and blocker view used by Compose and Library.
pub(crate) fn draw_composition_preview(
    ui: &mut egui::Ui,
    preview: &CompositionPreview,
    can_download_missing_items: bool,
    catalog: &[vapor_client_application::CatalogContent],
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
        ui.weak("Nothing has changed yet. Review what this setup needs before continuing.");
        if !can_download_missing_items {
            status_text(
                ui,
                "Getting missing items is paused until Studio finishes its current task and your saved project files are up to date.",
                StatusTone::Caution,
            );
        }
        for node in &preview.nodes {
            let source = match (node.source, node.already_installed) {
                (CompositionPreviewSource::Workspace, true) => {
                    "From project · already installed"
                }
                (CompositionPreviewSource::Workspace, false) => "From project",
                (CompositionPreviewSource::Cache, true) => "In your Library · already installed",
                (CompositionPreviewSource::Cache, false) => "In your Library",
                (CompositionPreviewSource::Installed, _) => {
                    "Already installed"
                }
                (CompositionPreviewSource::RootSeed, _) => "Available on Workshop",
            };
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} · {}",
                    super::super::display_content_kind(&node.kind),
                    super::super::display_catalog_identity(
                        &node.id,
                        node.version.as_deref(),
                        catalog,
                    ),
                ));
                ui.weak(source);
            });
            for dependency in &node.dependencies {
                ui.small(format!(
                    "↳ {} · {}{}",
                    dependency.relationship,
                    super::super::display_catalog_identity(
                        &dependency.id,
                        dependency.version.as_deref(),
                        catalog,
                    ),
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
                .filter(|_| node.source == CompositionPreviewSource::RootSeed)
                && !workshop_downloads.contains(published_file_id)
            {
                workshop_downloads.push(published_file_id.clone());
            }
        }
        if !workshop_downloads.is_empty() {
            ui.separator();
            ui.label(format!(
                "{} required Workshop item{} need to be added to your Library before this setup can be installed.",
                workshop_downloads.len(),
                if workshop_downloads.len() == 1 { "" } else { "s" }
            ));
            let download_clicked = ui
                .add_enabled(
                    can_download_missing_items,
                    egui::Button::new(format!(
                        "Add {} item{} to Library",
                        workshop_downloads.len(),
                        if workshop_downloads.len() == 1 { "" } else { "s" }
                    )),
                )
                .on_disabled_hover_text(
                    "Finish the current task and save or reload project edits before adding these items.",
                )
                .clicked();
            if !download_clicked {
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
