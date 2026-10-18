use super::super::{Section, StudioApp};
use super::design::{StatusTone, status_text};
use eframe::egui;
use std::path::PathBuf;

enum ContentDetailAction {
    StageWorkshop(String),
    ReviewPublish,
    ReviewComposition,
    AssignCompositionRoot(vapor_client_application::ContentReference, Option<PathBuf>),
}

impl StudioApp {
    #[allow(clippy::collapsible_if)]
    pub(in crate::app) fn draw_content_detail(&mut self, ui: &mut egui::Ui) {
        let mut action = None;
        if let Some(identity) = self.catalog.selected.as_ref() {
            if let Some(content) =
                self.catalog.items.iter().find(|content| {
                    super::super::catalog_content_selector_matches(content, identity)
                })
            {
                ui.heading("Content details");
                ui.label(format!(
                    "{} · {} · {}",
                    content.id,
                    super::super::display_content_kind(&content.kind),
                    content.version.as_deref().unwrap_or("unversioned")
                ));
                if !content.is_unambiguous() {
                    status_text(
                        ui,
                        "Copies on this device disagree. Review their details before using or publishing this content.",
                        StatusTone::Caution,
                    );
                    egui::CollapsingHeader::new("Conflicting copies").show(ui, |ui| {
                        for conflict in &content.conflicts {
                            ui.small(format!(
                                "{} differs between available copies: '{}' and '{}'",
                                conflict.field, conflict.first_value, conflict.conflicting_value,
                            ));
                        }
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    ui.weak("Available from");
                    for location in &content.locations {
                        ui.small(super::super::catalog::catalog_location_label(location));
                    }
                });
                egui::CollapsingHeader::new(format!(
                    "Included content · {}",
                    content.dependencies.len()
                ))
                .default_open(false)
                .show(ui, |ui| {
                    if content.dependencies.is_empty() {
                        ui.weak("No other content is required.");
                    } else {
                        for dependency in &content.dependencies {
                            ui.small(dependency);
                        }
                    }
                });
                let is_authored_here =
                    self.catalog.sources.iter().any(|source| {
                        super::super::source_content_selector_matches(source, identity)
                    });
                if is_authored_here {
                    let source_snapshot_current = self.catalog.source_snapshot_current();
                    let catalog_snapshot_current = self.catalog.local_catalog_current();
                    if ui.add_enabled(!self.operation.running && source_snapshot_current && catalog_snapshot_current && !self.has_unsaved_workspace_edits() && content.is_unambiguous(), egui::Button::new("Prepare for Steam Workshop"))
                        .on_disabled_hover_text(if !content.is_unambiguous() {
                            "Review the conflicting on-device copies before preparing this upload."
                        } else if !source_snapshot_current {
                            "Refresh the project list before preparing an upload from previous results."
                        } else if !catalog_snapshot_current {
                            "Refresh the on-device content list before preparing an upload; Studio cannot check for conflicts from previous results."
                        } else {
                            "Save or discard open project edits first; the Workshop review uses saved project files."
                        })
                        .clicked()
                    {
                        action = Some(ContentDetailAction::StageWorkshop(identity.clone()));
                    }
                }
                let preview = self.workshop.preview.as_ref().filter(|preview| {
                    is_authored_here
                        && preview.id == content.id
                        && preview.version.as_deref() == content.version.as_deref()
                });
                if let Some(preview) = preview {
                    ui.group(|ui| {
                        ui.heading("Workshop package staged");
                        ui.label(format!(
                            "{} · {} files · {} bytes · {}",
                            preview.id, preview.files, preview.bytes, preview.target
                        ));
                        if ui.button("Review in Publish").clicked() {
                            action = Some(ContentDetailAction::ReviewPublish);
                        }
                    });
                }
                if content.kind == vapor_core::ContentKind::Packagepack.as_str() {
                    let content_workspace = content
                        .locations
                        .iter()
                        .any(|location| {
                            location.kind == vapor_client_application::CatalogLocationKind::Workspace
                        })
                        .then(|| std::path::PathBuf::from(self.workspace.root.trim()));
                    let create_draft_blocks_assignment = self
                        .composition_root_change_blocked_by_create_draft(
                            content_workspace.as_deref(),
                        );
                    let selected_for_composition =
                        self.is_active_composition_root(&content.id, content.version.as_deref());
                    let catalog_snapshot_current = self.catalog.local_catalog_current();
                    if selected_for_composition {
                        ui.strong("This is the current composition");
                        if ui
                            .add_enabled(
                                !self.operation.running && !self.has_unsaved_workspace_edits(),
                                egui::Button::new("Review composition"),
                            )
                            .on_disabled_hover_text(
                                "Save or discard project edits before checking this composition.",
                            )
                            .clicked()
                        {
                            action = Some(ContentDetailAction::ReviewComposition);
                        }
                    } else if ui
                        .add_enabled(
                            !self.operation.running
                                && !self.has_unsaved_workspace_edits()
                                && !create_draft_blocks_assignment
                                && catalog_snapshot_current
                                && content.is_unambiguous(),
                                egui::Button::new("Use as the current composition"),
                        )
                        .on_disabled_hover_text(if !content.is_unambiguous() {
                            "Review the conflicting copies before selecting this content."
                        } else if !catalog_snapshot_current {
                            "Refresh the on-device content list before selecting from previous results."
                        } else if create_draft_blocks_assignment {
                            "Discard the Create draft before changing the active composition's workspace."
                        } else if self.has_unsaved_workspace_edits() {
                            "Save or discard project edits before changing the composition's project folder."
                        } else {
                            "Finish the current operation before changing the active composition."
                        })
                        .clicked()
                    {
                        action = Some(ContentDetailAction::AssignCompositionRoot(
                            content.identity(),
                            content_workspace,
                        ));
                    }
                }
            }
        }
        match action {
            Some(ContentDetailAction::StageWorkshop(identity)) => {
                self.stage_workshop_content(identity);
            }
            Some(ContentDetailAction::ReviewPublish) => {
                self.root_publishing.tab =
                    super::super::root_publishing::PublishTab::WorkshopContent;
                self.activate_navigation_section(Section::Publish);
            }
            Some(ContentDetailAction::ReviewComposition) => {
                self.activate_navigation_section(Section::Compositions);
                self.preview_selected_composition();
            }
            Some(ContentDetailAction::AssignCompositionRoot(identity, workspace)) => {
                self.assign_composition_root(identity, workspace);
            }
            None => {}
        }
    }
}
