use super::super::StudioApp;
use super::super::components::design::{StatusTone, page_header, primary_button, status_text};
use eframe::egui;
use std::path::PathBuf;
use vapor_studio_core::SourceCompositionLink;

#[derive(Clone, PartialEq, Eq)]
enum PackagepackRoleSelection {
    Empty,
    Choice(String),
    Unpinned,
    External,
}

impl StudioApp {
    pub(in crate::app) fn draw_composition(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Author", "Compose", "Build a content package by choosing its compatible projects and versions.");
        ui.weak("Saving checks the selected content and updates its source manifest through Vapor.");
        let composition_has_changes = self.authoring.composition_has_unsaved_changes();

        let source_projects = self
            .catalog
            .sources
            .iter()
            .map(|content| {
                (
                    super::super::source_content_selector(content),
                    content.kind.clone(),
                    content.version.clone(),
                )
            })
            .collect::<Vec<_>>();
        ui.horizontal(|ui| {
            ui.label("Source project");
            ui.add_enabled_ui(!self.operation.running && !composition_has_changes, |ui| {
                egui::ComboBox::from_id_salt("composition-source-project")
                    .selected_text(if self.authoring.composition_edit_selector.is_empty() {
                        "Select content".to_owned()
                    } else {
                        self.authoring.composition_edit_selector.clone()
                    })
                    .show_ui(ui, |ui| {
                        for (selector, kind, _) in &source_projects {
                            if ui
                                .selectable_label(
                                    self.authoring.composition_edit_selector == *selector,
                                    format!("{selector} · {kind}"),
                                )
                                .clicked()
                            {
                                self.authoring.composition_edit_selector = selector.clone();
                                self.authoring.composition_edit = None;
                                self.authoring.composition_original_links = None;
                                self.authoring.composition_link_candidate.clear();
                            }
                        }
                    });
            });
            if ui
                .add_enabled(
                    !self.operation.running
                        && !self.has_unsaved_workspace_edits()
                        && !self.authoring.composition_edit_selector.trim().is_empty(),
                    egui::Button::new("Load dependencies"),
                )
                .clicked()
            {
                self.load_source_composition();
            }
        });
        if composition_has_changes {
            ui.weak("Save or discard this draft before loading another source project.");
        }

        if source_projects.is_empty() {
            let candidates = self.workspace.unregistered_projects.candidates.len();
            let card_style = ui.style().clone();
            egui::Frame::group(&card_style).show(ui, |ui| {
                if candidates > 0 {
                    ui.heading(format!("{candidates} workspace project{} need adoption", if candidates == 1 { "" } else { "s" }));
                    ui.label("These manifests are already on this device. Adopt them into Vapor's content index to make them available in compositions and profiles.");
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Review and adopt projects")).clicked() {
                        self.navigate_to_workspace_content();
                    }
                } else if self.workspace.root.trim().is_empty() {
                    ui.heading("Open a workspace to compose content");
                    ui.label("Compositions connect authored Engine, Game, pack, and extension projects from one workspace.");
                    ui.horizontal(|ui| {
                        if ui.button("Open workspace").clicked() {
                            self.open_workspace_picker();
                        }
                        if ui.button("Create workspace").clicked() {
                            self.section = super::super::Section::Create;
                        }
                    });
                } else if !self.workspace.unregistered_scan_completed {
                    ui.heading("Check this workspace for content");
                    ui.label("A project scan has not completed yet. Scanning only reads manifests; it does not change workspace files.");
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Find workspace projects")).clicked() {
                        self.scan_unregistered_content_projects();
                    }
                } else {
                    ui.heading("No authored projects to compose yet");
                    ui.label("Create content in this workspace or scan again after adding a role manifest.");
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Create content")).clicked() {
                            self.section = super::super::Section::Create;
                        }
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Scan again")).clicked() {
                            self.scan_unregistered_content_projects();
                        }
                    });
                }
            });
        }

        let mut remove_link = None;
        let mut use_profile_root = false;
        let mut preview_source_selector = None;
        let mut stage_source_selector = None;
        let mut save_dependencies = false;
        let mut discard_dependencies = false;
        let mut source_graph_saved = false;
        let operation_running = self.operation.running;
        let source_contents = &self.catalog.sources;
        if let Some(composition) = self.authoring.composition_edit.as_mut() {
            ui.separator();
            ui.label(format!("Editing {} · {}", composition.id, composition.kind));
            ui.small(format!("Source file: {}", composition.manifest.display()));
            if composition.kind == vapor_core::ContentKind::Packagepack.as_str() {
                ui.add_enabled_ui(!operation_running, |ui| {
                    ui.group(|ui| {
                        ui.heading("Playable content");
                        ui.label("Choose the Engine that runs this composition and, optionally, the Game it contains.");
                        draw_packagepack_role_slot(
                            ui,
                            &mut composition.links,
                            &source_contents,
                            "engine-choice",
                            "Engine · required",
                            false,
                        );
                        draw_packagepack_role_slot(
                            ui,
                            &mut composition.links,
                            &source_contents,
                            "game-choice",
                            "Game · optional",
                            true,
                        );
                        ui.small("Save validates the complete composition through Vapor before writing its manifest.");
                    });
                });
            }
            egui::CollapsingHeader::new("Advanced dependency details")
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("Review version requirements, optional edges, and reasons directly.");
                    if composition.links.is_empty() {
                        ui.weak("No dependency edges are declared yet.");
                    }
                    ui.add_enabled_ui(!operation_running, |ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        egui::Grid::new("composition-edges")
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("Role");
                                ui.strong("Content");
                                ui.strong("Version requirement");
                                ui.strong("Optional");
                                ui.strong("Reason");
                                ui.end_row();
                                for (index, link) in composition.links.iter_mut().enumerate() {
                                    ui.label(&link.relationship);
                                    ui.label(&link.id);
                                    let mut version = link.version.clone().unwrap_or_default();
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(&mut version)
                                                .desired_width(150.0)
                                                .hint_text("any compatible version"),
                                        )
                                        .changed()
                                    {
                                        link.version = (!version.trim().is_empty()).then_some(version);
                                    }
                                    ui.checkbox(&mut link.optional, "");
                                    let mut reason = link.reason.clone().unwrap_or_default();
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(&mut reason)
                                                .desired_width(180.0)
                                                .hint_text("Why is this included?"),
                                        )
                                        .changed()
                                    {
                                        link.reason = (!reason.trim().is_empty()).then_some(reason);
                                    }
                                    if ui.small_button("Remove").clicked() {
                                        remove_link = Some(index);
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                    });
                });
            if let Some(index) = remove_link {
                composition.links.remove(index);
            }

            ui.separator();
            ui.label("Add additional workspace content");
            let choices = source_contents
                .iter()
                .filter(|content| content.id != composition.id)
                .filter(|content| {
                    match (
                        super::super::source_content_kind(&composition.kind),
                        super::super::source_content_kind(&content.kind),
                    ) {
                        (Some(parent), Some(child)) => {
                            vapor_studio_core::source_dependency_relationship(parent, child)
                                .is_some()
                                && !(parent == vapor_core::ContentKind::Packagepack
                                    && vapor_studio_core::source_dependency_choice_group(
                                        parent, child,
                                    )
                                    .is_some())
                        }
                        _ => false,
                    }
                })
                .map(|content| {
                    (
                        super::super::source_content_selector(content),
                        content.kind.clone(),
                        content.version.clone(),
                    )
                })
                .collect::<Vec<_>>();
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!operation_running, |ui| {
                    egui::ComboBox::from_id_salt("composition-link-candidate")
                        .selected_text(if self.authoring.composition_link_candidate.is_empty() {
                            "Choose additional content".to_owned()
                        } else {
                            self.authoring.composition_link_candidate.clone()
                        })
                        .show_ui(ui, |ui| {
                            for (selector, kind, version) in &choices {
                                let label = format!(
                                    "{selector} · {kind} · {}",
                                    version.as_deref().unwrap_or("unversioned")
                                );
                                ui.selectable_value(
                                    &mut self.authoring.composition_link_candidate,
                                    selector.clone(),
                                    label,
                                );
                            }
                    });
                });
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.authoring.composition_link_candidate.is_empty()
                            && choices.iter().any(|(selector, _, _)| {
                                selector == &self.authoring.composition_link_candidate
                            }),
                        egui::Button::new("Add content"),
                    )
                    .clicked()
                {
                    if let Some((selector, kind, version)) =
                        choices.iter().find(|(selector, _, _)| {
                            selector == &self.authoring.composition_link_candidate
                        })
                    {
                        let id = selector.split('@').next().unwrap_or(selector).to_owned();
                        let relationship = super::super::source_content_kind(&composition.kind)
                            .and_then(|parent| {
                                super::super::source_content_kind(kind).and_then(|child| {
                                    vapor_studio_core::source_dependency_relationship(parent, child)
                                })
                            })
                            .unwrap_or("dependency");
                        if composition
                            .links
                            .iter()
                            .any(|link| link.id == id && link.relationship == relationship)
                        {
                            self.status = format!("{id} is already included in this composition.");
                        } else {
                            composition.links.push(SourceCompositionLink {
                                id,
                                relationship: relationship.to_owned(),
                                optional: false,
                                version: version.as_ref().map(|version| format!("={version}")),
                                workshop_id: None,
                                reason: None,
                            });
                        }
                    }
                }
            });
            source_graph_saved =
                self.authoring.composition_original_links.as_ref() == Some(&composition.links);
            status_text(
                ui,
                if source_graph_saved {
                    "Dependencies match the saved manifest"
                } else {
                    "Unsaved dependency changes"
                },
                if source_graph_saved {
                    StatusTone::Positive
                } else {
                    StatusTone::Caution
                },
            );
            if !source_graph_saved {
                ui.add_enabled_ui(!self.operation.running, |ui| {
                    save_dependencies = primary_button(ui, "Save dependency changes").clicked();
                });
                discard_dependencies = ui
                    .add_enabled(!self.operation.running, egui::Button::new("Discard draft"))
                    .clicked();
            }
            if composition.kind == vapor_core::ContentKind::Packagepack.as_str() {
                ui.horizontal_wrapped(|ui| {
                    let source_selector = composition.version.as_ref()
                        .map(|version| format!("{}@{version}", composition.id))
                        .unwrap_or_else(|| composition.id.clone());
                    if ui.add_enabled(!self.operation.running && source_graph_saved, egui::Button::new("Preview availability")).clicked() {
                        preview_source_selector = Some(source_selector);
                    }
                    if ui.add_enabled(!self.operation.running && source_graph_saved, egui::Button::new("Use as active profile root"))
                        .on_disabled_hover_text("Save this composition before resolving it into a profile.")
                        .clicked()
                    {
                        use_profile_root = true;
                    }
                    ui.weak("This selects the source Packagepack and refreshes its resolved plan after save.");
                });
            }
            if ui
                .add_enabled(
                    !self.operation.running && source_graph_saved,
                    egui::Button::new("Stage Workshop review"),
                )
                .on_disabled_hover_text(
                    "Save this composition before staging its source content for review.",
                )
                .clicked()
            {
                stage_source_selector = Some(
                    composition
                        .version
                        .as_ref()
                        .map(|version| format!("{}@{version}", composition.id))
                        .unwrap_or_else(|| composition.id.clone()),
                );
            }
        } else {
            ui.separator();
            ui.weak("Load a source content project to inspect and edit its dependencies. Cache and installed copies remain read-only here.");
        }
        if save_dependencies {
            self.save_source_composition();
        }
        if discard_dependencies {
            self.discard_source_composition_changes();
        }
        if let Some(selector) = preview_source_selector {
            self.preview_source_composition(selector);
        }
        if use_profile_root {
            self.use_edited_composition_for_profile();
        }
        if let Some(selector) = stage_source_selector {
            self.catalog.selected = Some(selector.clone());
            self.section = super::super::Section::Discover;
            self.stage_workshop_content(selector);
        }
        let selector = self.authoring.composition_edit.as_ref().map(|composition| {
            composition
                .version
                .as_ref()
                .map(|version| format!("{}@{version}", composition.id))
                .unwrap_or_else(|| composition.id.clone())
        });
        let preview = self.profile_composition.preview().cloned();
        if source_graph_saved
            && let (Some(selector), Some(preview)) = (selector, preview)
            && preview.selector == selector
        {
            let published_file_ids =
                super::super::components::composition_preview::draw_composition_preview(
                    ui, &preview,
                );
            if !published_file_ids.is_empty() {
                self.download_source_composition_dependencies(
                    published_file_ids,
                    selector.clone(),
                    PathBuf::from(self.workspace.root.trim()),
                );
            }
        }
        ui.add_space(6.0);
        ui.small(format!(
            "Workspace: {}",
            PathBuf::from(self.workspace.root.trim()).display()
        ));
    }
}

fn packagepack_relationship_group(relationship: &str) -> Option<&'static str> {
    match relationship {
        "engine" | "enginepack" => Some("engine-choice"),
        "game" | "gamepack" => Some("game-choice"),
        _ => None,
    }
}

fn draw_packagepack_role_slot(
    ui: &mut egui::Ui,
    links: &mut Vec<SourceCompositionLink>,
    sources: &[vapor_studio_core::SourceContent],
    group: &'static str,
    label: &str,
    optional: bool,
) {
    let packagepack = vapor_core::ContentKind::Packagepack;
    let choices = sources
        .iter()
        .filter_map(|content| {
            let kind = super::super::source_content_kind(&content.kind)?;
            if vapor_studio_core::source_dependency_choice_group(packagepack, kind)
                != Some(group)
            {
                return None;
            }
            let relationship =
                vapor_studio_core::source_dependency_relationship(packagepack, kind)?;
            Some((
                super::super::source_content_selector(content),
                content.kind.clone(),
                content.version.clone(),
                relationship.to_owned(),
            ))
        })
        .collect::<Vec<_>>();
    let current_link = links
        .iter()
        .find(|link| packagepack_relationship_group(&link.relationship) == Some(group));
    let current_choice = current_link.and_then(|link| {
        choices
            .iter()
            .find(|(selector, _, version, _)| {
                let id = selector.split('@').next().unwrap_or(selector);
                if id != link.id {
                    return false;
                }
                match (&link.version, version) {
                    (None, None) => true,
                    (Some(requirement), Some(version)) => {
                        requirement == &format!("={version}") || requirement == version
                    }
                    _ => false,
                }
            })
            .map(|(selector, _, _, _)| selector.clone())
    });
    let current_id_is_local = current_link.is_some_and(|link| {
        choices.iter().any(|(selector, _, _, _)| {
            selector.split('@').next().unwrap_or(selector) == link.id
        })
    });
    let mut selected = current_link.map_or(PackagepackRoleSelection::Empty, |_| {
        current_choice.map_or_else(
            || {
                if current_id_is_local
                    && current_link.is_some_and(|link| {
                        link.version.as_deref().is_none_or(|requirement| requirement == "*")
                    })
                {
                    PackagepackRoleSelection::Unpinned
                } else {
                    PackagepackRoleSelection::External
                }
            },
            PackagepackRoleSelection::Choice,
        )
    });
    let initial_selection = selected.clone();
    let current_label = match (&current_link, &selected) {
        (Some(link), PackagepackRoleSelection::External) => format!(
            "External content · {} · {}",
            link.id,
            link.version.as_deref().unwrap_or("any version"),
        ),
        (Some(link), PackagepackRoleSelection::Unpinned) => {
            format!("Version not pinned · {}", link.id)
        }
        (Some(_), PackagepackRoleSelection::Choice(selector)) => choices
            .iter()
            .find(|choice| &choice.0 == selector)
            .map(|choice| packagepack_choice_label(&choice.0, &choice.1, choice.2.as_deref()))
            .unwrap_or_else(|| selector.clone()),
        (Some(_), PackagepackRoleSelection::Empty) if optional => "No Game".to_owned(),
        (Some(_), PackagepackRoleSelection::Empty) => "Choose an Engine".to_owned(),
        (None, _) if optional => "No Game".to_owned(),
        (None, _) => "Choose an Engine".to_owned(),
    };

    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(("packagepack-role", group))
            .width(340.0)
            .selected_text(current_label)
            .show_ui(ui, |ui| {
                if optional {
                    ui.selectable_value(
                        &mut selected,
                        PackagepackRoleSelection::Empty,
                        "No Game",
                    );
                }
                for (selector, kind, version, _) in &choices {
                    ui.selectable_value(
                        &mut selected,
                        PackagepackRoleSelection::Choice(selector.clone()),
                        packagepack_choice_label(selector, kind, version.as_deref()),
                    );
                }
            });
    });

    if selected != initial_selection {
        links.retain(|link| packagepack_relationship_group(&link.relationship) != Some(group));
        if let PackagepackRoleSelection::Choice(selector) = selected
            && let Some((_, _, version, relationship)) =
                choices.iter().find(|choice| choice.0 == selector)
        {
            let id = selector.split('@').next().unwrap_or(&selector).to_owned();
            links.push(SourceCompositionLink {
                id,
                relationship: relationship.clone(),
                optional: false,
                version: version.as_ref().map(|version| format!("={version}")),
                workshop_id: None,
                reason: None,
            });
        }
    }

    if choices.is_empty() && !optional {
        ui.weak("No Engine or Enginepack is registered in this workspace yet. Create or adopt one first.");
    }
}

fn packagepack_choice_label(selector: &str, kind: &str, version: Option<&str>) -> String {
    format!(
        "{} · {kind} · {}",
        selector.split('@').next().unwrap_or(selector),
        version.map_or_else(|| "unversioned".to_owned(), |value| format!("v{value}")),
    )
}
