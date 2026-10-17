use super::super::{Section, StudioApp};
use eframe::egui;
use std::path::PathBuf;

enum ContentDetailAction {
    StageWorkshop(String),
    ReviewPublish,
    AssignProfileRoot(vapor_studio_core::ContentIdentity, Option<PathBuf>),
}

impl StudioApp {
    #[allow(clippy::collapsible_if)]
    pub(in crate::app) fn draw_content_detail(&mut self, ui: &mut egui::Ui) {
        let mut action = None;
        if let Some(identity) = self.catalog.selected.as_ref() {
            if let Some(content) = self
                .catalog
                .items
                .iter()
                .find(|content| super::super::catalog_content_selector_matches(content, identity))
            {
                ui.heading("Content details");
                ui.label(format!(
                    "{} · {} · {}",
                    content.id,
                    content.kind,
                    content.version.as_deref().unwrap_or("unversioned")
                ));
                if !content.is_unambiguous() {
                    ui.colored_label(egui::Color32::YELLOW, "Local copies disagree. Review the source details before using or publishing this content.");
                    egui::CollapsingHeader::new("Conflicting source details").show(ui, |ui| {
                        for conflict in &content.conflicts {
                            ui.small(format!(
                                "{} differs: '{}' at {} · '{}' at {}",
                                conflict.field,
                                conflict.first_value,
                                conflict.first_location.display(),
                                conflict.conflicting_value,
                                conflict.conflicting_location.display(),
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
                egui::CollapsingHeader::new("File locations and integrity details").show(
                    ui,
                    |ui| {
                        for location in &content.locations {
                            let source =
                                super::super::catalog::catalog_location_label(location);
                            ui.label(format!(
                                "{source}{} · {}",
                                location
                                    .install_source
                                    .as_deref()
                                    .map(|source| format!(" · via {source}"))
                                    .unwrap_or_default(),
                                location.root.display()
                            ));
                            ui.small(format!(
                                "File fingerprint: {}",
                                location
                                    .fingerprint
                                    .as_deref()
                                    .unwrap_or("not recorded for this source")
                            ));
                        }
                    },
                );
                egui::CollapsingHeader::new(format!(
                    "Dependencies · {}",
                    content.dependencies.len()
                ))
                .default_open(false)
                .show(ui, |ui| {
                    if content.dependencies.is_empty() {
                        ui.weak("No declared dependencies.");
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
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && content.is_unambiguous(), egui::Button::new("Stage Workshop review package"))
                        .on_disabled_hover_text(if content.is_unambiguous() { "Save or discard workspace file and structured metadata drafts first; staging reads the workspace source from disk." } else { "Resolve conflicting local records before staging this identity." })
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
                            location.kind == vapor_studio_core::CatalogLocationKind::Workspace
                        })
                        .then(|| std::path::PathBuf::from(self.workspace.root.trim()));
                    let create_draft_blocks_assignment = self
                        .profile_root_change_blocked_by_create_draft(content_workspace.as_deref());
                    let profile_root = self.profile.items[self.profile.active_index]
                        .root_package
                        .as_ref();
                    let selected_for_profile = profile_root.is_some_and(|root| {
                        root.id == content.id && root.version == content.version
                    });
                    if selected_for_profile {
                        ui.strong("Selected for this profile");
                    } else if ui
                        .add_enabled(
                            !self.operation.running
                                && !self.has_unsaved_workspace_edits()
                                && !create_draft_blocks_assignment
                                && content.is_unambiguous(),
                            egui::Button::new("Use this version for the active profile"),
                        )
                        .on_disabled_hover_text(if !content.is_unambiguous() {
                            "Resolve the identity conflict before selecting this content."
                        } else if create_draft_blocks_assignment {
                            "Discard the Create draft before changing the active profile's workspace."
                        } else if self.has_unsaved_workspace_edits() {
                            "Save or discard workspace edits before changing the active profile's source workspace."
                        } else {
                            "Finish the current operation before changing the active profile."
                        })
                        .clicked()
                    {
                        action = Some(ContentDetailAction::AssignProfileRoot(
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
                self.section = Section::Publish;
            }
            Some(ContentDetailAction::AssignProfileRoot(identity, workspace)) => {
                self.assign_profile_root(identity, workspace);
            }
            None => {}
        }
    }
}
