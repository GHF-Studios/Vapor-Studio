use eframe::egui;
use super::super::{StudioApp, catalog_matches_filters};
use vapor_studio_core::ContentIdentity;

impl StudioApp {
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
                    self.workshop_preview = None;
                }
                let location_summary = content.locations.iter().map(|location| {
                    let source = match location.kind {
                        vapor_studio_core::CatalogLocationKind::Workspace => "Workspace",
                        vapor_studio_core::CatalogLocationKind::Cache => "Cache",
                        vapor_studio_core::CatalogLocationKind::Installed if location.enabled => "Installed",
                        vapor_studio_core::CatalogLocationKind::Installed => "Installed · disabled",
                    };
                    format!("{source}: {}", location.root.display())
                }).collect::<Vec<_>>().join(" · ");
                ui.small(location_summary);
                if selected && kind == vapor_core::ContentKind::Packagepack.as_str()
                    && ui.add_enabled(!self.running && !self.workspace_editor_dirty,
                        egui::Button::new("Use this exact version as the profile root"))
                        .on_disabled_hover_text("Save or discard editor changes before switching the profile's workspace.")
                        .clicked()
                {
                    let root = ContentIdentity {
                        id: id.to_owned(), version: content.version.clone(),
                    };
                    let workspace = content.locations.iter()
                        .find(|location| location.kind == vapor_studio_core::CatalogLocationKind::Workspace)
                        .map(|location| location.root.clone());
                    self.assign_profile_root(root, workspace);
                }
                ui.separator();
            }
        });
        if let Some(identity) = self.selected_catalog_content.clone() {
            if let Some(content) = self.catalog.iter().find(|content| super::super::catalog_content_selector(content) == identity).cloned() {
                ui.heading("Content details");
                ui.label(format!("{} · {} · {}", content.id, content.kind, content.version.as_deref().unwrap_or("unversioned")));
                for location in &content.locations {
                    let source = match location.kind {
                        vapor_studio_core::CatalogLocationKind::Workspace => "Workspace",
                        vapor_studio_core::CatalogLocationKind::Cache => "Vapor cache",
                        vapor_studio_core::CatalogLocationKind::Installed if location.enabled => "Installed",
                        vapor_studio_core::CatalogLocationKind::Installed => "Installed · disabled",
                    };
                    ui.label(format!("{source} · {}", location.root.display()));
                }
                if content.dependencies.is_empty() {
                    ui.weak("No declared dependencies.");
                } else {
                    ui.label("Dependencies");
                    for dependency in &content.dependencies {
                        ui.small(dependency);
                    }
                }
                let is_authored_here = self.contents.iter().any(|source| {
                    super::super::source_content_selector(source) == identity
                });
                if is_authored_here {
                    if ui.add_enabled(!self.running && !self.workspace_editor_dirty, egui::Button::new("Stage Workshop review package"))
                        .on_disabled_hover_text("Save or reload the open file first; staging reads the workspace source from disk.")
                        .clicked()
                    {
                        self.stage_workshop_content(identity.clone());
                    }
                }
                let preview = self.workshop_preview.as_ref().filter(|preview| {
                    is_authored_here && preview.id == content.id
                        && preview.version.as_deref() == content.version.as_deref()
                }).cloned();
                if let Some(preview) = preview {
                    ui.group(|ui| {
                        ui.heading("Staged artifact review");
                        ui.label(format!("Title: {}", preview.title.as_deref().unwrap_or(&preview.id)));
                        ui.label(format!("Steam AppID: {} · visibility: {}", preview.app_id.map_or_else(|| "missing".to_owned(), |id| id.to_string()), preview.visibility.as_deref().unwrap_or("not set")));
                        ui.label(format!("PublishedFileId: {}", preview.published_file_id.as_deref().unwrap_or("new item")));
                        ui.label(format!("Target: {} · {} files · {} bytes", preview.target, preview.files, preview.bytes));
                        ui.label(format!("Payload: {}", preview.root.display()));
                        ui.small(format!("Fingerprint: {}", preview.fingerprint));
                        ui.collapsing("Staged files", |ui| {
                            for path in preview.file_paths.iter().take(30) { ui.small(path); }
                            if preview.file_paths.len() > 30 {
                                ui.weak(format!("{} more files", preview.file_paths.len() - 30));
                            }
                        });
                        if preview.dependencies.is_empty() { ui.weak("No declared dependencies."); }
                        else { for dependency in &preview.dependencies { ui.small(format!("Requires {dependency}")); } }
                        ui.separator();
                        ui.strong(if preview.published_file_id.is_some() { "Publish update to Workshop" } else { "Create and publish Workshop item" });
                        ui.add_enabled(!self.running, egui::TextEdit::singleline(&mut self.workshop_account).hint_text("Steam publisher account"));
                        ui.add_enabled(!self.running, egui::TextEdit::singleline(&mut self.workshop_change_note).hint_text("Change note (optional)"));
                        let confirmation = format!("PUBLISH {}", preview.id);
                        ui.label(format!("This uploads to Steam. Type {confirmation} to confirm."));
                        ui.add_enabled(!self.running, egui::TextEdit::singleline(&mut self.workshop_confirmation).hint_text(&confirmation));
                        if ui.add_enabled(
                            !self.running && !self.workshop_account.trim().is_empty() && self.workshop_confirmation.trim() == confirmation,
                            egui::Button::new(if preview.published_file_id.is_some() { "Publish update" } else { "Create Workshop item" }),
                        ).clicked() {
                            self.publish_reviewed_workshop_content(identity.clone(), preview.fingerprint.clone());
                        }
                    });
                }
                if content.kind == vapor_core::ContentKind::Packagepack.as_str() {
                    let profile_root = self.profiles[self.active_profile].root_package.as_ref();
                    if profile_root.is_some_and(|root| root.id == content.id && root.version == content.version) {
                        ui.strong("Selected for this profile");
                    }
                }
            }
        }
    }

}
