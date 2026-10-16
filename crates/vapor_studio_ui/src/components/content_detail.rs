use super::super::{Section, StudioApp, catalog_matches_filters};
use eframe::egui;
use vapor_studio_core::ContentIdentity;

impl StudioApp {
    #[allow(clippy::collapsible_if)]
    pub(in crate::app) fn draw_content_list(&mut self, ui: &mut egui::Ui) {
        if self.catalog.items.is_empty() {
            ui.weak("No workspace, cached, or installed content is available locally.");
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for content in self.catalog.items.clone() {
                let id = content.id.as_str();
                let version = content.version.clone().unwrap_or_else(|| "unversioned".to_owned());
                let kind = content.kind.as_str();
                let identity = super::super::catalog_content_selector(&content);
                if !catalog_matches_filters(
                    &content,
                    self.catalog.source_filter,
                    self.catalog.kind_filter.as_deref(),
                    &self.catalog.search_query,
                ) { continue; }
                let selected = self.catalog.selected.as_deref() == Some(identity.as_str());
                if ui.selectable_label(selected, format!("{id} · {kind} · {version}")).clicked() {
                    self.catalog.selected = Some(identity.clone());
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
                    && ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && content.is_unambiguous(),
                        egui::Button::new("Use this exact version as the profile root"))
                        .on_disabled_hover_text(if content.is_unambiguous() { "Save or discard workspace file and structured metadata drafts before switching the profile's workspace." } else { "Resolve conflicting local records before selecting this content." })
                        .clicked()
                {
                    let root = ContentIdentity {
                        id: id.to_owned(), version: content.version.clone(),
                    };
                    let workspace = content
                        .locations
                        .iter()
                        .any(|location| location.kind == vapor_studio_core::CatalogLocationKind::Workspace)
                        .then(|| std::path::PathBuf::from(self.workspace.root.trim()));
                    self.assign_profile_root(root, workspace);
                }
                ui.separator();
            }
        });
        if let Some(identity) = self.catalog.selected.clone() {
            if let Some(content) = self
                .catalog
                .items
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
                    .catalog
                    .sources
                    .iter()
                    .any(|source| super::super::source_content_selector(source) == identity);
                if is_authored_here {
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && content.is_unambiguous(), egui::Button::new("Stage Workshop review package"))
                        .on_disabled_hover_text(if content.is_unambiguous() { "Save or discard workspace file and structured metadata drafts first; staging reads the workspace source from disk." } else { "Resolve conflicting local records before staging this identity." })
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
                        ui.heading("Workshop package staged");
                        ui.label(format!(
                            "{} · {} files · {} bytes · {}",
                            preview.id, preview.files, preview.bytes, preview.target
                        ));
                        if ui.button("Review in Publish").clicked() {
                            self.section = Section::Publish;
                        }
                    });
                }
                if content.kind == vapor_core::ContentKind::Packagepack.as_str() {
                    let profile_root = self.profile.items[self.profile.active_index].root_package.as_ref();
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
