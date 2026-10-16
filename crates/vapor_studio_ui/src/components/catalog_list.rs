use super::super::{StudioApp, catalog_matches_filters};
use eframe::egui;
use vapor_studio_core::ContentIdentity;

impl StudioApp {
    #[allow(clippy::collapsible_if)]
    pub(in crate::app) fn draw_catalog_list(&mut self, ui: &mut egui::Ui) {
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
    }
}
