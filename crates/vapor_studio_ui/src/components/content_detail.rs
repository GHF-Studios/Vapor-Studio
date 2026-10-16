use super::super::{StudioApp, catalog_matches_filters};
use eframe::egui;
use vapor_studio_core::ContentIdentity;

impl StudioApp {
    #[allow(clippy::collapsible_if)]
    pub(in crate::app) fn draw_content_list(&mut self, ui: &mut egui::Ui) {
        if self.catalog.is_empty() {
            ui.weak("No workspace, cached, or installed content is available locally.");
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for content in self.catalog.clone() {
                let id = content.id.as_str();
                let version = content.version.clone().unwrap_or_else(|| "unversioned".to_owned());
                let kind = content.kind.as_str();
                let identity = super::super::catalog_content_selector(&content);
                if !catalog_matches_filters(
                    &content,
                    self.catalog_source_filter,
                    self.catalog_kind_filter.as_deref(),
                    &self.search_query,
                ) { continue; }
                let selected = self.selected_catalog_content.as_deref() == Some(identity.as_str());
                if ui.selectable_label(selected, format!("{id} · {kind} · {version}")).clicked() {
                    self.selected_catalog_content = Some(identity.clone());
                    self.workshop.preview = None;
                }
                if !content.is_unambiguous() {
                    ui.colored_label(egui::Color32::YELLOW, "Identity conflict");
                }
                let location_summary = content.locations.iter().map(|location| {
                    let source = match location.kind {
                        vapor_studio_core::CatalogLocationKind::Workspace => "Workspace",
                        vapor_studio_core::CatalogLocationKind::Cache => "Cache",
                        vapor_studio_core::CatalogLocationKind::Installed if location.enabled => "Installed",
                        vapor_studio_core::CatalogLocationKind::Installed => "Installed · disabled",
                    };
                    format!(
                        "{source}{}: {}",
                        location
                            .install_source
                            .as_deref()
                            .map(|source| format!(" · via {source}"))
                            .unwrap_or_default(),
                        location.root.display()
                    )
                }).collect::<Vec<_>>().join(" · ");
                ui.small(location_summary);
                if selected && kind == vapor_core::ContentKind::Packagepack.as_str()
                    && ui.add_enabled(!self.running && !self.workspace_editor_dirty && content.is_unambiguous(),
                        egui::Button::new("Use this exact version as the profile root"))
                        .on_disabled_hover_text(if content.is_unambiguous() { "Save or discard editor changes before switching the profile's workspace." } else { "Resolve conflicting local records before selecting this content." })
                        .clicked()
                {
                    let root = ContentIdentity {
                        id: id.to_owned(), version: content.version.clone(),
                    };
                    let workspace = content
                        .locations
                        .iter()
                        .any(|location| location.kind == vapor_studio_core::CatalogLocationKind::Workspace)
                        .then(|| std::path::PathBuf::from(self.workspace_root.trim()));
                    self.assign_profile_root(root, workspace);
                }
                ui.separator();
            }
        });
        if let Some(identity) = self.selected_catalog_content.clone() {
            if let Some(content) = self
                .catalog
                .iter()
                .find(|content| super::super::catalog_content_selector(content) == identity)
                .cloned()
            {
                ui.heading("Content details");
                ui.label(format!(
                    "{} · {} · {}",
                    content.id,
                    content.kind,
                    content.version.as_deref().unwrap_or("unversioned")
                ));
                if !content.is_unambiguous() {
                    ui.colored_label(egui::Color32::YELLOW, "Conflicting local records: do not select or publish this identity until its sources are reconciled.");
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
                }
                for location in &content.locations {
                    let source = match location.kind {
                        vapor_studio_core::CatalogLocationKind::Workspace => "Workspace",
                        vapor_studio_core::CatalogLocationKind::Cache => "Vapor cache",
                        vapor_studio_core::CatalogLocationKind::Installed if location.enabled => {
                            "Installed"
                        }
                        vapor_studio_core::CatalogLocationKind::Installed => "Installed · disabled",
                    };
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
                        "Payload fingerprint: {}",
                        location
                            .fingerprint
                            .as_deref()
                            .unwrap_or("not recorded for this source")
                    ));
                }
                if content.dependencies.is_empty() {
                    ui.weak("No declared dependencies.");
                } else {
                    ui.label("Dependencies");
                    for dependency in &content.dependencies {
                        ui.small(dependency);
                    }
                }
                let is_authored_here = self
                    .contents
                    .iter()
                    .any(|source| super::super::source_content_selector(source) == identity);
                if is_authored_here {
                    if ui.add_enabled(!self.running && !self.workspace_editor_dirty && content.is_unambiguous(), egui::Button::new("Stage Workshop review package"))
                        .on_disabled_hover_text(if content.is_unambiguous() { "Save or reload the open file first; staging reads the workspace source from disk." } else { "Resolve conflicting local records before staging this identity." })
                        .clicked()
                    {
                        self.stage_workshop_content(identity.clone());
                    }
                }
                let preview = self
                    .workshop.preview
                    .as_ref()
                    .filter(|preview| {
                        is_authored_here
                            && preview.id == content.id
                            && preview.version.as_deref() == content.version.as_deref()
                    })
                    .cloned();
                if let Some(preview) = preview {
                    ui.group(|ui| {
                        ui.heading("Staged artifact review");
                        ui.label(format!(
                            "Title: {}",
                            preview.title.as_deref().unwrap_or(&preview.id)
                        ));
                        ui.label(format!(
                            "Steam AppID: {} · visibility: {}",
                            preview
                                .app_id
                                .map_or_else(|| "missing".to_owned(), |id| id.to_string()),
                            preview.visibility.as_deref().unwrap_or("not set")
                        ));
                        ui.label(format!(
                            "PublishedFileId: {}",
                            preview.published_file_id.as_deref().unwrap_or("new item")
                        ));
                        ui.label(format!(
                            "Target: {} · {} files · {} bytes",
                            preview.target, preview.files, preview.bytes
                        ));
                        ui.label(format!("Payload: {}", preview.root.display()));
                        ui.small(format!("Fingerprint: {}", preview.fingerprint));
                        ui.collapsing("Staged files", |ui| {
                            for path in preview.file_paths.iter().take(30) {
                                ui.small(path);
                            }
                            if preview.file_paths.len() > 30 {
                                ui.weak(format!("{} more files", preview.file_paths.len() - 30));
                            }
                        });
                        if preview.dependencies.is_empty() {
                            ui.weak("No declared dependencies.");
                        } else {
                            for dependency in &preview.dependencies {
                                ui.small(format!("Requires {dependency}"));
                            }
                        }
                        ui.separator();
                        ui.strong(if preview.published_file_id.is_some() {
                            "Publish update to Workshop"
                        } else {
                            "Create and publish Workshop item"
                        });
                        ui.add_enabled(
                            !self.running && content.is_unambiguous(),
                            egui::TextEdit::singleline(&mut self.workshop.publisher_account)
                                .hint_text("Steam publisher account"),
                        );
                        ui.add_enabled(
                            !self.running && content.is_unambiguous(),
                            egui::TextEdit::singleline(&mut self.workshop.change_note)
                                .hint_text("Change note (optional)"),
                        );
                        let confirmation = format!("PUBLISH {}", preview.id);
                        ui.label(format!(
                            "This uploads to Steam. Type {confirmation} to confirm."
                        ));
                        ui.add_enabled(
                            !self.running,
                            egui::TextEdit::singleline(&mut self.workshop.confirmation)
                                .hint_text(&confirmation),
                        );
                        if ui
                            .add_enabled(
                                !self.running
                                    && content.is_unambiguous()
                                    && !self.workshop.publisher_account.trim().is_empty()
                                    && self.workshop.confirmation.trim() == confirmation,
                                egui::Button::new(if preview.published_file_id.is_some() {
                                    "Publish update"
                                } else {
                                    "Create Workshop item"
                                }),
                            )
                            .clicked()
                        {
                            self.publish_reviewed_workshop_content(
                                identity.clone(),
                                preview,
                            );
                        }
                    });
                }
                if content.kind == vapor_core::ContentKind::Packagepack.as_str() {
                    let profile_root = self.profiles[self.active_profile].root_package.as_ref();
                    if profile_root.is_some_and(|root| {
                        root.id == content.id && root.version == content.version
                    }) {
                        ui.strong("Selected for this profile");
                    }
                }
            }
        }
    }
}
