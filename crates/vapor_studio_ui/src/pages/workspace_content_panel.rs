use super::super::StudioApp;
use super::super::components::design::{
    StatusTone, confirmation_card, search_field_width, status_text,
};
use eframe::egui;
use std::path::{Path, PathBuf};

enum SourceContentAction {
    Validate(vapor_client_application::SourceContent),
    Package(vapor_client_application::SourceContent, bool),
    OpenManifest(PathBuf),
    EditDependencies(String),
    UseAsCompositionRoot(vapor_client_application::ContentReference, PathBuf),
    ReviewComposition,
    EditWorkshopMetadata(vapor_client_application::SourceContent),
    EditRuntimeOutputs(vapor_client_application::SourceContent),
    Unregister(vapor_client_application::SourceContent),
}

impl StudioApp {
    pub(super) fn draw_workspace_content_search(&mut self, ui: &mut egui::Ui) -> String {
        ui.horizontal_wrapped(|ui| {
            let search_width = search_field_width(ui);
            ui.add(
                egui::TextEdit::singleline(&mut self.workspace.content_search)
                    .desired_width(search_width)
                    .hint_text("Find a project by ID, role, or version"),
            );
            if !self.workspace.content_search.is_empty() && ui.small_button("Clear").clicked() {
                self.workspace.content_search.clear();
            }
        });
        self.workspace.content_search.trim().to_lowercase()
    }

    pub(super) fn draw_workspace_content_panel(&mut self, ui: &mut egui::Ui, query: &str) {
        self.draw_last_package_result(ui);
        if let Some(error) = self.catalog.source_discovery_error.as_deref() {
            status_text(
                ui,
                &format!(
                    "Projects in this folder could not refresh; {}. {error}",
                    if self.catalog.source_discovery_loaded {
                        "showing the last successful list"
                    } else {
                        "no successful list is available yet"
                    }
                ),
                StatusTone::Caution,
            );
        }
        if let Some(error) = self.workspace.unregistered_scan_error.as_deref() {
            status_text(
                ui,
                &format!(
                    "Existing-project scan failed; {}. {error}",
                    if self.workspace.unregistered_scan_completed {
                        "showing its last successful results"
                    } else {
                        "no successful results are available yet"
                    }
                ),
                StatusTone::Caution,
            );
        }
        if self.catalog.sources.is_empty() {
            let card_style = ui.style().clone();
            egui::Frame::group(&card_style).show(ui, |ui| {
                if !self.workspace.unregistered_projects.candidates.is_empty() {
                    if self.catalog.source_snapshot_current() {
                        ui.heading(format!(
                            "{} existing content project{} found",
                            self.workspace.unregistered_projects.candidates.len(),
                            if self.workspace.unregistered_projects.candidates.len() == 1 { "" } else { "s" },
                        ));
                        ui.label("Nothing has been changed. Add a project below to make it available in Studio; its files stay where they are.");
                    } else {
                        ui.heading(format!(
                            "{} possible content project{} found",
                            self.workspace.unregistered_projects.candidates.len(),
                            if self.workspace.unregistered_projects.candidates.len() == 1 { "" } else { "s" },
                        ));
                        ui.label("Studio cannot confirm which folders have already been added. Refresh the project list before adding one of these.");
                    }
                } else if self.workspace.unregistered_scan_completed {
                    if self.catalog.source_snapshot_current() {
                        ui.heading("No projects in this folder yet");
                        ui.label("Create a project from a template, or add an existing project folder to Studio.");
                    } else {
                        ui.heading("The project list could not be confirmed");
                        ui.label("The folder scan finished, but Studio could not refresh its project list. Refresh before treating this folder as empty.");
                    }
                } else {
                    ui.heading("Find existing projects");
                    ui.label("Studio checks this folder for projects you can add. Adding a project keeps its files where they are.");
                }
                ui.horizontal_wrapped(|ui| {
                    if self.catalog.source_discovery_error.is_some()
                        && ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Refresh project list")).clicked()
                    {
                        self.discover();
                    }
                    if self.catalog.source_snapshot_current()
                        && !self.workspace.unregistered_scan_completed
                        && ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Find existing projects")).clicked()
                    {
                        self.scan_unregistered_content_projects();
                    }
                    if ui.add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_workspace_edits()
                            && self.catalog.source_snapshot_current(),
                        egui::Button::new("Create content"),
                    ).on_disabled_hover_text("Refresh the project list before starting a new project.").clicked() {
                        self.activate_navigation_section(super::super::Section::Create);
                    }
                    if self.catalog.source_snapshot_current()
                        && self.workspace.unregistered_scan_completed
                        && ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Scan again")).clicked()
                    {
                        self.scan_unregistered_content_projects();
                    }
                });
            });
        } else {
            ui.heading(format!(
                "Projects in this folder · {}",
                self.catalog.sources.len()
            ));
            ui.label("Choose a project to check its files, change the content it needs, or prepare it for sharing.");
            let mut visible_projects = 0;
            let mut action = None;
            for content in self.catalog.sources.iter().filter(|content| {
                query.is_empty()
                    || content.id.to_lowercase().contains(query)
                    || content.kind.to_lowercase().contains(query)
                    || content
                        .version
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(query)
            }) {
                visible_projects += 1;
                ui.group(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(&content.id);
                            ui.weak(super::super::display_content_kind(&content.kind));
                            ui.weak(content.version.as_deref().unwrap_or("project version"));
                        });
                        egui::CollapsingHeader::new(format!(
                            "Project details · {} dependencies · {} built files",
                            content.dependencies.len(),
                            content.binaries.len() + content.libraries.len(),
                        ))
                            .default_open(false)
                            .show(ui, |ui| {
                                if !content.binaries.is_empty() || !content.libraries.is_empty() {
                                    let mut outputs = Vec::new();
                                    if !content.binaries.is_empty() {
                                        outputs.push(format!("binaries: {}", content.binaries.join(", ")));
                                    }
                                    if !content.libraries.is_empty() {
                                        outputs.push(format!("libraries: {}", content.libraries.join(", ")));
                                    }
                                    ui.small(format!("Built files · {}", outputs.join(" · ")));
                                }
                                if !content.dependencies.is_empty() {
                                    ui.small(format!("Includes: {}", content.dependencies.join(" · ")));
                                }
                                let workshop = &content.workshop;
                                if workshop.app_id.is_some()
                                    || workshop.published_file_id.is_some()
                                    || workshop.visibility.is_some()
                                    || workshop.title.is_some()
                                    || workshop.description.is_some()
                                    || !workshop.tags.is_empty()
                                    || workshop.change_note.is_some()
                                {
                                    ui.collapsing("Workshop publication metadata", |ui| {
                                        if let Some(app_id) = workshop.app_id {
                                            ui.small(format!("Steam app: {app_id}"));
                                        } else {
                                            status_text(
                                                ui,
                                                "Steam app not configured",
                                                StatusTone::Caution,
                                            );
                                        }
                                        ui.small(format!(
                                            "Published item: {}",
                                            workshop.published_file_id.as_deref().unwrap_or("new item")
                                        ));
                                        if let Some(visibility) = &workshop.visibility {
                                            ui.small(format!("Visibility: {visibility}"));
                                        }
                                        if let Some(title) = &workshop.title {
                                            ui.small(format!("Workshop title: {title}"));
                                        }
                                        if let Some(description) = &workshop.description {
                                            ui.small(format!("Description: {description}"));
                                        }
                                        if !workshop.tags.is_empty() {
                                            ui.small(format!("Tags: {}", workshop.tags.join(", ")));
                                        }
                                        if let Some(change_note) = &workshop.change_note {
                                            ui.small(format!("Default change note: {change_note}"));
                                        }
                                        ui.weak("These settings describe the package; its built files are checked before upload.");
                                    });
                                } else {
                                    ui.small("Workshop publication: not configured");
                                }
                            });
                        ui.horizontal_wrapped(|ui| {
                            let relative_manifest = content
                                .manifest
                                .strip_prefix(Path::new(self.workspace.root.trim()))
                                .ok()
                                .map(PathBuf::from);
                            if ui
                                .add_enabled(
                                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new("Check project"),
                                )
                                .on_hover_text("Check that this project and its required content are set up correctly.")
                                .clicked()
                            {
                                action = Some(SourceContentAction::Validate(content.clone()));
                            }
                            if ui
                                .add_enabled(
                                    !self.operation.running && !self.has_unsaved_workspace_edits(),
                                    egui::Button::new("Build package"),
                                )
                                .on_hover_text("Build this workspace, then stage a reusable package in this Vapor installation. This does not publish it.")
                                .clicked()
                            {
                                action = Some(SourceContentAction::Package(content.clone(), false));
                            }
                            if let Some(relative_manifest) = relative_manifest
                                && ui
                                    .add_enabled(
                                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                                        egui::Button::new("Edit project file"),
                                    )
                                    .on_disabled_hover_text("Save or discard the current file before opening the project definition.")
                                    .clicked()
                            {
                                action = Some(SourceContentAction::OpenManifest(relative_manifest));
                            }
                            if ui
                                    .add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Choose included content"))
                                    .clicked()
                            {
                                action = Some(SourceContentAction::EditDependencies(
                                    super::super::source_content_selector(content),
                                ));
                            }
                            if content.kind == vapor_client_protocol::ContentKind::Packagepack.as_str() {
                                let selected_as_composition_root = self.is_active_composition_root(
                                    &content.id,
                                    content.version.as_deref(),
                                );
                                if selected_as_composition_root {
                                    ui.strong("Used by this composition");
                                    if ui
                                        .add_enabled(
                                            !self.operation.running
                                                && !self.has_unsaved_workspace_edits(),
                                            egui::Button::new("Review composition"),
                                        )
                                        .clicked()
                                    {
                                        action = Some(SourceContentAction::ReviewComposition);
                                    }
                                } else if ui
                                    .add_enabled(
                                        !self.operation.running
                                            && self.catalog.source_snapshot_current()
                                            && !self.has_unsaved_workspace_edits(),
                                        egui::Button::new("Use in this composition"),
                                    )
                                    .clicked()
                                {
                                    action = Some(SourceContentAction::UseAsCompositionRoot(
                                        vapor_client_application::ContentReference {
                                            id: content.id.clone(),
                                            version: content.version.clone(),
                                        },
                                        PathBuf::from(self.workspace.root.trim()),
                                    ));
                                }
                            }
                            ui.menu_button("More", |ui| {
                                if ui
                                    .add_enabled(
                                        !self.operation.running
                                            && !self.has_unsaved_workspace_edits(),
                                        egui::Button::new("Preview package"),
                                    )
                                    .on_hover_text(
                                        "Build this workspace and assemble a temporary package preview without keeping a staged copy.",
                                    )
                                    .clicked()
                                {
                                    action = Some(SourceContentAction::Package(
                                        content.clone(),
                                        true,
                                    ));
                                    ui.close();
                                }
                                ui.separator();
                                if ui
                                    .add_enabled(
                                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                                        egui::Button::new("Edit Workshop metadata"),
                                    )
                                    .on_disabled_hover_text("Save or discard your open edits first; Workshop changes use the latest saved project details.")
                                    .clicked()
                                {
                                    action = Some(SourceContentAction::EditWorkshopMetadata(
                                        content.clone(),
                                    ));
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                                        egui::Button::new("Edit built files"),
                                    )
                                    .on_disabled_hover_text("Save or discard your open edits first; Studio reads the latest saved project details.")
                                    .clicked()
                                {
                                    action = Some(SourceContentAction::EditRuntimeOutputs(
                                        content.clone(),
                                    ));
                                    ui.close();
                                }
                                ui.separator();
                                if ui
                                    .add_enabled(
                                        !self.operation.running && !self.has_unsaved_workspace_edits(),
                                        egui::Button::new("Stop using in Studio"),
                                    )
                                    .on_hover_text("Remove it from Studio's project list. Its files stay where they are.")
                                    .clicked()
                                {
                                    action = Some(SourceContentAction::Unregister(content.clone()));
                                    ui.close();
                                }
                            });
                        });
                    });
            }
            ui.small(format!(
                "Showing {visible_projects} of {} projects in this folder",
                self.catalog.sources.len()
            ));
            if visible_projects == 0 {
                ui.weak("No projects in this folder match this search.");
            }
            match action {
                Some(SourceContentAction::Validate(content)) => {
                    self.validate_source_project(&content);
                }
                Some(SourceContentAction::Package(content, dry_run)) => {
                    self.package_source_project(&content, dry_run);
                }
                Some(SourceContentAction::OpenManifest(path)) => {
                    self.open_workspace_file(path);
                }
                Some(SourceContentAction::EditDependencies(selector)) => {
                    if self.authoring.pending_composition_role_change.is_some() {
                        self.status = "Confirm or cancel the pending composition change before opening another one.".to_owned();
                    } else {
                        self.authoring.composition_edit_selector = selector;
                        self.authoring.composition_edit = None;
                        self.authoring.composition_original_links = None;
                        self.activate_navigation_section(super::super::Section::Compose);
                        self.load_source_composition();
                    }
                }
                Some(SourceContentAction::UseAsCompositionRoot(identity, workspace)) => {
                    self.assign_composition_root(identity, Some(workspace));
                }
                Some(SourceContentAction::ReviewComposition) => {
                    self.activate_navigation_section(super::super::Section::Compositions);
                    self.preview_selected_composition();
                }
                Some(SourceContentAction::EditWorkshopMetadata(content)) => {
                    self.load_source_workshop_metadata(&content);
                }
                Some(SourceContentAction::EditRuntimeOutputs(content)) => {
                    self.load_source_runtime_outputs(&content);
                }
                Some(SourceContentAction::Unregister(content)) => {
                    self.workspace.pending_unregistration = Some(content);
                }
                None => {}
            }
        }
        let mut unregistration_confirmation = None;
        if let Some(project) = self.workspace.pending_unregistration.as_ref() {
            let title = format!("Remove {} from Studio?", project.id);
            let description = "Its files stay where they are, and you can add it again later.";
            unregistration_confirmation = confirmation_card(
                ui,
                &title,
                description,
                "Remove from Studio",
                "Keep project",
                !self.operation.running && !self.has_unsaved_workspace_edits(),
            );
        }
        match unregistration_confirmation {
            Some(true) => self.unregister_content_project(),
            Some(false) => self.workspace.pending_unregistration = None,
            None => {}
        }
    }

    fn draw_last_package_result(&mut self, ui: &mut egui::Ui) {
        let Some((workspace_root, id, selector, target, files, bytes, dry_run)) = self
            .workspace
            .last_package
            .as_ref()
            .filter(|result| result.workspace_root == Path::new(self.workspace.root.trim()))
            .map(|result| {
                (
                    result.workspace_root.clone(),
                    result.id.clone(),
                    result.selector.clone(),
                    result.target.clone(),
                    result.files,
                    result.bytes,
                    result.dry_run,
                )
            })
        else {
            return;
        };
        let (title, color) = if dry_run {
            (
                "Package preview complete",
                egui::Color32::from_rgb(120, 190, 230),
            )
        } else {
            (
                "Package built and staged",
                egui::Color32::from_rgb(130, 210, 150),
            )
        };
        let workshop_selector = (!dry_run)
            .then(|| {
                self.catalog
                    .sources
                    .iter()
                    .find(|content| {
                        super::super::source_content_selector(content) == selector
                            && content.workshop.app_id.is_some()
                            && content.root.starts_with(&workspace_root)
                    })
                    .map(super::super::source_content_selector)
            })
            .flatten();
        let can_review_workshop = !self.operation.running
            && !self.has_unsaved_workspace_edits()
            && self.catalog.source_snapshot_current()
            && self.catalog.local_catalog_current();
        let already_in_library = is_cached_exact_version(&self.catalog.items, &selector);
        let can_add_to_library = !dry_run
            && !already_in_library
            && !self.operation.running
            && !self.has_unsaved_workspace_edits()
            && self.catalog.source_snapshot_current();
        let (review_workshop, acquire_to_library) = egui::Frame::group(ui.style())
            .show(ui, |ui| {
            ui.colored_label(color, title);
            ui.horizontal_wrapped(|ui| {
                ui.strong(&id);
                ui.weak(format!(
                    "{} · {} files · {}",
                    display_target(&target),
                    files,
                    format_bytes(bytes)
                ));
            });
            if !dry_run {
                ui.small("Package built for this device. It has not been published.");
                ui.horizontal_wrapped(|ui| {
                    let acquire = if already_in_library {
                        ui.strong("Available in Library");
                        false
                    } else {
                        ui.add_enabled(can_add_to_library, egui::Button::new("Add to Library"))
                            .on_disabled_hover_text(if self.has_unsaved_workspace_edits() {
                                "Save the source edits before adding this package."
                            } else if !self.catalog.source_snapshot_current() {
                                "Refresh the source project list before adding this package."
                            } else {
                                "Wait for the current operation to finish."
                            })
                            .on_hover_text("Make this built version available to add to a composition.")
                            .clicked()
                    };
                    let review = if workshop_selector.is_some() {
                        ui.weak("Continue to sharing after reviewing the current saved source.");
                        ui.add_enabled(can_review_workshop, egui::Button::new("Review Workshop upload"))
                            .on_disabled_hover_text(if self.has_unsaved_workspace_edits() {
                                "Save or discard project edits before preparing the review."
                            } else if !self.catalog.source_snapshot_current()
                                || !self.catalog.local_catalog_current()
                            {
                                "Refresh the project and on-device content lists before preparing the review."
                            } else {
                                "Wait for the current operation to finish."
                            })
                            .on_hover_text("Build the latest saved project and prepare a Workshop review before uploading.")
                            .clicked()
                    } else {
                        false
                    };
                    (review, acquire)
                }).inner
            } else {
                ui.small("Preview only; no package was saved.");
                (false, false)
            }
        })
            .inner;
        if review_workshop && let Some(selector) = workshop_selector {
            self.root_publishing.tab = super::super::root_publishing::PublishTab::WorkshopContent;
            self.stage_workshop_content(selector);
            self.activate_navigation_section(super::super::Section::Publish);
        }
        if acquire_to_library {
            self.acquire_packaged_source(selector, workspace_root);
        }
    }

    pub(super) fn draw_unregistered_projects_panel(&mut self, ui: &mut egui::Ui, query: &str) {
        if !self.workspace.root.trim().is_empty() {
            let (has_candidates, adoption_path) = {
                let candidates = &self.workspace.unregistered_projects.candidates;
                let matching_candidates = candidates
                    .iter()
                    .filter(|candidate| {
                        query.is_empty()
                            || candidate.id.to_lowercase().contains(query)
                            || candidate.kind.to_string().to_lowercase().contains(query)
                            || candidate
                                .path
                                .to_string_lossy()
                                .to_lowercase()
                                .contains(query)
                    })
                    .collect::<Vec<_>>();
                let has_candidates = !candidates.is_empty();
                if !matching_candidates.is_empty() {
                    ui.heading(format!(
                        "Projects to add · {} of {}",
                        matching_candidates.len(),
                        candidates.len()
                    ));
                    ui.weak("Add a project to make it available in Studio. Its files stay where they are.");
                }
                let mut adoption_path = None;
                for candidate in &matching_candidates {
                    let card_style = ui.style().clone();
                    egui::Frame::group(&card_style).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.vertical(|ui| {
                                ui.strong(&candidate.id);
                                ui.small(format!(
                                    "{} · {}",
                                    super::super::display_content_kind(&candidate.kind.to_string()),
                                    candidate
                                        .path
                                        .strip_prefix(std::path::Path::new(
                                            self.workspace.root.trim()
                                        ))
                                        .unwrap_or(&candidate.path)
                                        .display()
                                ));
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_enabled(
                                            !self.operation.running
                                                && self.workspace.unregistered_scan_error.is_none()
                                                && !self.has_unsaved_workspace_edits(),
                                            egui::Button::new("Add project"),
                                        )
                                        .on_disabled_hover_text(if self.workspace.unregistered_scan_error.is_some() {
                                            "Refresh the folder scan before adding a project; the current results may be out of date."
                                        } else {
                                            "Save or discard your edits before adding this project."
                                        })
                                        .clicked()
                                    {
                                        adoption_path = Some(candidate.path.clone());
                                    }
                                },
                            );
                        });
                    });
                }
                if has_candidates && matching_candidates.is_empty() {
                    ui.weak("No other projects match this search.");
                }
                (has_candidates, adoption_path)
            };
            if let Some(path) = adoption_path {
                self.adopt_content_project(path);
            }
            if self.workspace.unregistered_scan_completed && !has_candidates {
                ui.weak("No other project folders were found.");
            }
            if !self.workspace.unregistered_scan_completed
                && !has_candidates
                && !self.catalog.sources.is_empty()
            {
                ui.horizontal_wrapped(|ui| {
                    ui.weak("A project scan has not completed yet.");
                    if ui
                        .add_enabled(
                            !self.operation.running && !self.has_unsaved_workspace_edits(),
                            egui::Button::new("Find projects"),
                        )
                        .clicked()
                    {
                        self.scan_unregistered_content_projects();
                    }
                });
            }
            for diagnostic in &self.workspace.unregistered_projects.diagnostics {
                status_text(ui, diagnostic, StatusTone::Caution);
            }
        }
    }
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

pub(super) fn display_target(target: &str) -> String {
    target
        .split(',')
        .map(|target| {
            let parts = target.split('-').collect::<Vec<_>>();
            match parts.as_slice() {
                [architecture, _, os, ..] => {
                    let os = match *os {
                        "linux" => "Linux",
                        "windows" => "Windows",
                        "darwin" => "macOS",
                        "freebsd" => "FreeBSD",
                        other => other,
                    };
                    format!("{os} · {architecture}")
                }
                _ => target.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn is_cached_exact_version(
    items: &[vapor_client_application::CatalogContent],
    selector: &str,
) -> bool {
    let (id, version) = selector
        .rsplit_once('@')
        .map(|(id, version)| (id, Some(version)))
        .unwrap_or((selector, None));
    items.iter().any(|item| {
        item.id == id
            && item.version.as_deref() == version
            && item.locations.iter().any(|location| {
                location.kind == vapor_client_application::CatalogLocationKind::Cache
            })
    })
}

#[cfg(test)]
mod package_library_tests {
    use super::is_cached_exact_version;
    use vapor_client_application::{CatalogContent, CatalogLocation, CatalogLocationKind};

    #[test]
    fn library_presence_matches_the_exact_content_version() {
        let items = [CatalogContent {
            id: "ghf.example.mod".to_owned(),
            name: Some("Example".to_owned()),
            version: Some("1.2.3".to_owned()),
            kind: "extension".to_owned(),
            published_file_id: None,
            dependencies: Vec::new(),
            locations: vec![CatalogLocation {
                kind: CatalogLocationKind::Cache,
                root: "/vapor/cache/example".into(),
                enabled: true,
                fingerprint: None,
                install_source: None,
            }],
            conflicts: Vec::new(),
        }];

        assert!(is_cached_exact_version(&items, "ghf.example.mod@1.2.3"));
        assert!(!is_cached_exact_version(&items, "ghf.example.mod@1.2.4"));
        assert!(!is_cached_exact_version(&items, "ghf.example.mod"));
    }
}
