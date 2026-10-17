use super::super::StudioApp;
use super::super::components::design::{
    StatusTone, discard_confirmation, page_header, primary_button, status_text,
};
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
        page_header(
            ui,
            "Game setup",
            "Choose what this profile launches",
            "Choose the game engine, game, and add-ons that belong in this playable setup.",
        );
        ui.weak(
            "This chooses what the selected play profile launches. Your project files stay in their project folders.",
        );
        if let Some(error) = self.catalog.source_discovery_error.as_deref() {
            status_text(
                ui,
                &format!(
                    "The project list could not refresh, so some results may be out of date. Refresh it before loading dependencies, reviewing this setup, or changing the active profile. {error}"
                ),
                StatusTone::Caution,
            );
        }
        let composition_has_changes = self.authoring.composition_has_unsaved_changes();
        let composition_role_change_pending =
            self.authoring.pending_composition_role_change.is_some();

        self.draw_composition_source_selector(
            ui,
            composition_has_changes,
            composition_role_change_pending,
        );

        if self.catalog.sources.is_empty() {
            self.draw_composition_empty_state(ui);
        }

        let mut remove_link = None;
        let mut use_profile_root = false;
        let mut review_profile_plan = false;
        let mut preview_source_selector = None;
        let mut stage_source_selector = None;
        let mut save_dependencies = false;
        let mut discard_dependencies = false;
        let mut source_graph_saved = false;
        let operation_running = self.operation.running;
        let workspace_manifest_saved =
            !self.workspace.editor.has_dirty_documents() && !self.authoring.manifest_metadata_dirty;
        let active_profile_root = self
            .profile
            .items
            .get(self.profile.active_index)
            .and_then(|profile| profile.root_package.clone());
        let source_contents = &self.catalog.sources;
        if let Some(composition) = self.authoring.composition_edit.as_mut() {
            ui.separator();
            ui.label(format!(
                "Editing {} · {}",
                composition.id,
                super::super::display_content_kind(&composition.kind)
            ));
            egui::CollapsingHeader::new("Project file details")
                .default_open(false)
                .show(ui, |ui| {
                    ui.small(format!(
                        "Definition file: {}",
                        composition.manifest.display()
                    ))
                });
            if composition.kind == vapor_core::ContentKind::Packagepack.as_str() {
                ui.add_enabled_ui(!operation_running && !composition_role_change_pending, |ui| {
                    ui.group(|ui| {
                        ui.heading("Playable content");
                        ui.label("Choose one game engine and one game for this setup.");
                        draw_packagepack_role_slot(
                            ui,
                            PackagepackRoleSlotSpec {
                                group: "engine-choice",
                                label: "Game engine · required",
                                empty_label: "Choose an Engine",
                                optional: false,
                            },
                            PackagepackRoleSlotContext {
                                links: &mut composition.links,
                                sources: source_contents,
                                search: &mut self.authoring.packagepack_engine_search,
                                pending_change: &mut self.authoring.pending_composition_role_change,
                            },
                        );
                        draw_packagepack_role_slot(
                            ui,
                            PackagepackRoleSlotSpec {
                                group: "game-choice",
                                label: "Game · required",
                                empty_label: "Choose a Game",
                                optional: false,
                            },
                            PackagepackRoleSlotContext {
                                links: &mut composition.links,
                                sources: source_contents,
                                search: &mut self.authoring.packagepack_game_search,
                                pending_change: &mut self.authoring.pending_composition_role_change,
                            },
                        );
                        ui.small("Save checks that both required roles are available before updating this setup.");
                    });
                },
            );
            }
            egui::CollapsingHeader::new("Advanced dependency details")
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("Review version requirements, optional edges, and reasons directly.");
                    if composition.links.is_empty() {
                        ui.weak("No dependency edges are declared yet.");
                    }
                    ui.add_enabled_ui(
                        !operation_running && !composition_role_change_pending,
                        |ui| {
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
                                        for (index, link) in
                                            composition.links.iter_mut().enumerate()
                                        {
                                            ui.label(&link.relationship);
                                            ui.label(&link.id);
                                            let mut version =
                                                link.version.clone().unwrap_or_default();
                                            if ui
                                                .add(
                                                    egui::TextEdit::singleline(&mut version)
                                                        .desired_width(150.0)
                                                        .hint_text("any compatible version"),
                                                )
                                                .changed()
                                            {
                                                link.version =
                                                    (!version.trim().is_empty()).then_some(version);
                                            }
                                            ui.checkbox(&mut link.optional, "");
                                            let mut reason =
                                                link.reason.clone().unwrap_or_default();
                                            if ui
                                                .add(
                                                    egui::TextEdit::singleline(&mut reason)
                                                        .desired_width(180.0)
                                                        .hint_text("Why is this included?"),
                                                )
                                                .changed()
                                            {
                                                link.reason =
                                                    (!reason.trim().is_empty()).then_some(reason);
                                            }
                                            if ui.small_button("Remove").clicked() {
                                                remove_link = Some(index);
                                            }
                                            ui.end_row();
                                        }
                                    });
                            });
                        },
                    );
                });
            if let Some(index) = remove_link {
                composition.links.remove(index);
            }

            ui.separator();
            ui.label("Add other content from these project folders");
            ui.horizontal_wrapped(|ui| {
                let search_width = super::super::components::design::search_field_width(ui);
                ui.add(
                    egui::TextEdit::singleline(&mut self.authoring.composition_link_search)
                        .desired_width(search_width)
                        .hint_text("Search by name, role, or version"),
                );
                if !self.authoring.composition_link_search.is_empty()
                    && ui.small_button("Clear").clicked()
                {
                    self.authoring.composition_link_search.clear();
                }
            });
            let link_query = self.authoring.composition_link_search.trim().to_lowercase();
            let compatible_candidates = source_contents
                .iter()
                .filter(|content| {
                    is_additional_composition_dependency(content, composition)
                        && source_content_matches_query(content, &link_query)
                })
                .count();
            ui.small(format!(
                "{compatible_candidates} compatible project{}",
                if compatible_candidates == 1 { "" } else { "s" }
            ));
            if compatible_candidates == 0 {
                ui.weak(if link_query.is_empty() {
                    "No additional content roles are compatible with this project."
                } else {
                    "No compatible projects match this search."
                });
            }
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(
                    !operation_running && !composition_role_change_pending,
                    |ui| {
                        egui::ComboBox::from_id_salt("composition-link-candidate")
                            .selected_text(
                                if self.authoring.composition_link_candidate.is_empty() {
                                    "Choose additional content".to_owned()
                                } else {
                                    self.authoring.composition_link_candidate.clone()
                                },
                            )
                            .show_ui(ui, |ui| {
                                for content in source_contents.iter().filter(|content| {
                                    is_additional_composition_dependency(content, composition)
                                        && source_content_matches_query(content, &link_query)
                                }) {
                                    let selector = super::super::source_content_selector(content);
                                    let label = format!(
                                        "{selector} · {} · {}",
                                        super::super::display_content_kind(&content.kind),
                                        content.version.as_deref().unwrap_or("unversioned")
                                    );
                                    if ui
                                        .selectable_label(
                                            self.authoring.composition_link_candidate == selector,
                                            label,
                                        )
                                        .clicked()
                                    {
                                        self.authoring.composition_link_candidate = selector;
                                    }
                                }
                            });
                    },
                );
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !composition_role_change_pending
                            && !self.authoring.composition_link_candidate.is_empty()
                            && source_contents.iter().any(|content| {
                                is_additional_composition_dependency(content, composition)
                                    && super::super::source_content_selector_matches(
                                        content,
                                        &self.authoring.composition_link_candidate,
                                    )
                            }),
                        egui::Button::new("Add content"),
                    )
                    .clicked()
                    && let Some(content) = source_contents.iter().find(|content| {
                        is_additional_composition_dependency(content, composition)
                            && super::super::source_content_selector_matches(
                                content,
                                &self.authoring.composition_link_candidate,
                            )
                    })
                {
                    let id = content.id.clone();
                    let relationship = super::super::source_content_kind(&composition.kind)
                        .and_then(|parent| {
                            super::super::source_content_kind(&content.kind).and_then(|child| {
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
                            version: content
                                .version
                                .as_ref()
                                .map(|version| format!("={version}")),
                            workshop_id: None,
                            reason: None,
                        });
                    }
                }
            });
            source_graph_saved =
                self.authoring.composition_original_links.as_ref() == Some(&composition.links);
            status_text(
                ui,
                if source_graph_saved {
                    "Included content matches the saved project"
                } else {
                    "Unsaved content changes"
                },
                if source_graph_saved {
                    StatusTone::Positive
                } else {
                    StatusTone::Caution
                },
            );
            if !source_graph_saved {
                ui.add_enabled_ui(
                    !self.operation.running && !composition_role_change_pending,
                    |ui| {
                        save_dependencies = primary_button(ui, "Save content choices").clicked();
                    },
                );
                if !self.authoring.composition_discard_confirmation_open {
                    discard_dependencies = ui
                        .add_enabled(
                            !self.operation.running && !composition_role_change_pending,
                            egui::Button::new("Discard draft"),
                        )
                        .clicked();
                }
            }
            if composition.kind == vapor_core::ContentKind::Packagepack.as_str() {
                ui.horizontal_wrapped(|ui| {
                    let source_selector = composition.version.as_ref()
                        .map(|version| format!("{}@{version}", composition.id))
                        .unwrap_or_else(|| composition.id.clone());
                    if ui
                        .add_enabled(
                            !operation_running
                                && !composition_role_change_pending
                                && source_graph_saved
                                && workspace_manifest_saved
                                && self.catalog.source_snapshot_current(),
                            egui::Button::new("Preview availability"),
                        )
                        .on_disabled_hover_text(if !workspace_manifest_saved {
                            "Save or reload dirty workspace files and structured manifest changes first; the preview reads saved files."
                        } else {
                            "Save the content choices and confirm the pending game role before previewing."
                        })
                        .clicked()
                    {
                        preview_source_selector = Some(source_selector);
                    }
                    let already_profile_root = active_profile_root.as_ref().is_some_and(|root| {
                        root.id == composition.id && root.version == composition.version
                    });
                    if already_profile_root {
                        ui.strong("Already selected for the active profile");
                        if ui
                            .add_enabled(
                                !operation_running
                                    && !composition_role_change_pending
                                    && source_graph_saved
                                    && workspace_manifest_saved,
                                egui::Button::new("Review play setup"),
                            )
                            .on_disabled_hover_text(if !workspace_manifest_saved {
                                "Save or reload dirty workspace files and structured manifest changes before resolving the profile."
                            } else {
                                "Resolve the pending role choice and save composition changes first."
                            })
                            .clicked()
                        {
                            review_profile_plan = true;
                        }
                    } else if ui
                        .add_enabled(
                            !operation_running
                                && !composition_role_change_pending
                                && source_graph_saved
                                && workspace_manifest_saved
                                && self.catalog.source_snapshot_current(),
                            egui::Button::new("Use as active profile's game setup"),
                        )
                        .on_disabled_hover_text(if !workspace_manifest_saved {
                            "Save or reload dirty workspace files and structured manifest changes before changing the profile's game setup."
                        } else {
                            "Save this composition before resolving it into a profile."
                        })
                        .clicked()
                    {
                        use_profile_root = true;
                    }
                    ui.weak("Choosing a different game setup refreshes what this profile will install and launch.");
                });
            }
            if ui
                .add_enabled(
                    !self.operation.running
                        && !composition_role_change_pending
                        && source_graph_saved
                        && workspace_manifest_saved
                        && self.catalog.source_snapshot_current()
                        && self.catalog.local_catalog_current(),
                    egui::Button::new("Stage Workshop review"),
                )
                .on_disabled_hover_text(if !self.catalog.source_snapshot_current() {
                    "Refresh workspace sources successfully before staging from the last-known project list."
                } else if !self.catalog.local_catalog_current() {
                    "Refresh the on-device content list before preparing this for Steam Workshop; Studio cannot check for conflicts using out-of-date information."
                } else if !workspace_manifest_saved {
                    "Save or reload dirty workspace files and structured manifest changes before staging this source."
                } else {
                    "Save this composition before staging its source content for review."
                })
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
            ui.weak("Choose a project to review what it includes. Downloaded and installed copies are managed separately.");
        }
        if let Some(change) = self.authoring.pending_composition_role_change.clone() {
            let proposal_available = change.proposed.as_ref().is_none_or(|proposal| {
                source_contents.iter().any(|content| {
                    if content.id != proposal.id {
                        return false;
                    }
                    let relationship =
                        super::super::source_content_kind(&content.kind).and_then(|kind| {
                            vapor_studio_core::source_dependency_relationship(
                                vapor_core::ContentKind::Packagepack,
                                kind,
                            )
                        });
                    relationship == Some(proposal.relationship.as_str())
                        && proposal.version.as_deref().is_none_or(|requirement| {
                            requirement == "*"
                                || content.version.as_deref().is_some_and(|version| {
                                    requirement == version || requirement == format!("={version}")
                                })
                        })
                })
            });
            let previous = change
                .previous
                .iter()
                .map(|link| format!("{} · {}", link.relationship, link.id))
                .collect::<Vec<_>>()
                .join(", ");
            let proposed = change.proposed.as_ref().map_or_else(
                || "No selection".to_owned(),
                |link| format!("{} · {}", link.relationship, link.id),
            );
            let role = if change.group == "engine-choice" {
                "Engine"
            } else {
                "Game"
            };
            let removes_choice = change.proposed.is_none();
            let title = if removes_choice {
                format!("Remove the current {role} choice?")
            } else {
                format!("Replace the current {role} choice?")
            };
            let description = if proposal_available {
                format!(
                    "Change {previous} to {proposed}? Other required content stays as it is. This updates the draft only; save the project afterward to keep the change."
                )
            } else {
                format!(
                    "{proposed} is no longer available at the reviewed version. Keep the current choice, refresh the project list, and choose it again."
                )
            };
            if let Some(confirm) = super::super::components::design::confirmation_card(
                ui,
                &title,
                &description,
                if removes_choice {
                    "Remove role choice"
                } else {
                    "Replace role choice"
                },
                "Keep current choice",
                !self.operation.running && proposal_available,
            ) {
                if confirm && let Some(composition) = self.authoring.composition_edit.as_mut() {
                    composition.links.retain(|link| {
                        packagepack_relationship_group(&link.relationship) != Some(change.group)
                    });
                    if let Some(proposed) = change.proposed {
                        composition.links.push(proposed);
                    }
                }
                self.authoring.pending_composition_role_change = None;
            }
        }
        if save_dependencies {
            self.save_source_composition();
        }
        if discard_dependencies {
            self.authoring.composition_discard_confirmation_open = true;
        }
        if self.authoring.composition_discard_confirmation_open
            && let Some(discard) = discard_confirmation(
                ui,
                "Discard content changes?",
                "The current content choices will return to the values saved in this project.",
            )
        {
            match discard {
                true => {
                    self.discard_source_composition_changes();
                }
                false => {
                    self.authoring.composition_discard_confirmation_open = false;
                }
            }
        }
        if let Some(selector) = preview_source_selector {
            self.preview_source_composition(selector);
        }
        if use_profile_root {
            self.use_edited_composition_for_profile();
        }
        if review_profile_plan {
            self.activate_navigation_section(super::super::Section::Library);
            self.preview_selected_composition();
        }
        if let Some(selector) = stage_source_selector {
            self.select_catalog_identity(selector.clone());
            self.open_discover_tab(super::super::catalog::DiscoverTab::Local);
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
                    ui,
                    &preview,
                    !operation_running && workspace_manifest_saved,
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

    fn draw_composition_empty_state(&mut self, ui: &mut egui::Ui) {
        let candidates = self.workspace.unregistered_projects.candidates.len();
        let card_style = ui.style().clone();
        egui::Frame::group(&card_style).show(ui, |ui| {
                if candidates > 0 {
                    ui.heading(format!("{candidates} project{} to add", if candidates == 1 { "" } else { "s" }));
                    ui.label("These projects are already in the open folder. Add them to Studio to use them in a game setup; their files will stay where they are.");
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Review projects")).clicked() {
                        self.navigate_to_workspace_content();
                    }
                } else if self.workspace.root.trim().is_empty() {
                    ui.heading("Open a project folder to set up a game");
                    ui.label("A game setup combines an engine, a game, and optional add-ons from your project folders.");
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Open project folder").clicked() {
                            self.open_workspace_picker();
                        }
                        if ui.button("Create project folder").clicked() {
                            self.activate_navigation_section(super::super::Section::Create);
                        }
                    });
                } else if !self.workspace.unregistered_scan_completed {
                    ui.heading("Find projects in this folder");
                    ui.label("Studio only checks project information; it does not change your files.");
                    if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Find projects")).clicked() {
                        self.scan_unregistered_content_projects();
                    }
                } else {
                    ui.heading("No game content projects found yet");
                    ui.label("Create a project from a template, or add an existing one to this folder.");
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Create content")).clicked() {
                            self.activate_navigation_section(super::super::Section::Create);
                        }
                        if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits(), egui::Button::new("Scan again")).clicked() {
                            self.scan_unregistered_content_projects();
                        }
                    });
                }
            });
    }

    fn draw_composition_source_selector(
        &mut self,
        ui: &mut egui::Ui,
        composition_has_changes: bool,
        composition_role_change_pending: bool,
    ) {
        let source_query = self
            .authoring
            .composition_source_search
            .trim()
            .to_lowercase();
        let source_match_count = self
            .catalog
            .sources
            .iter()
            .filter(|content| source_content_matches_query(content, &source_query))
            .count();
        let selected_project_label = self
            .catalog
            .sources
            .iter()
            .find(|content| {
                super::super::source_content_selector(content)
                    == self.authoring.composition_edit_selector
            })
            .map(|content| {
                format!(
                    "{} · {}",
                    content.id,
                    super::super::display_content_kind(&content.kind)
                )
            })
            .unwrap_or_else(|| {
                if self.authoring.composition_edit_selector.trim().is_empty() {
                    "Choose a project".to_owned()
                } else {
                    self.authoring.composition_edit_selector.clone()
                }
            });
        ui.horizontal_wrapped(|ui| {
            ui.label("Project to edit");
            ui.add_enabled_ui(
                !self.operation.running
                    && self.catalog.source_snapshot_current()
                    && !composition_has_changes
                    && !composition_role_change_pending,
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(
                                &mut self.authoring.composition_source_search,
                            )
                            .desired_width(super::super::components::design::search_field_width(ui))
                            .hint_text("Find a project"),
                        );
                        if !self.authoring.composition_source_search.is_empty()
                            && ui.small_button("Clear").clicked()
                        {
                            self.authoring.composition_source_search.clear();
                        }
                        egui::ComboBox::from_id_salt("composition-source-project")
                            .selected_text(selected_project_label)
                            .show_ui(ui, |ui| {
                                for content in self.catalog.sources.iter().filter(|content| {
                                    source_content_matches_query(content, &source_query)
                                }) {
                                    let selector = super::super::source_content_selector(content);
                                    if ui
                                        .selectable_label(
                                            self.authoring.composition_edit_selector == selector,
                                            format!(
                                                "{selector} · {}",
                                                super::super::display_content_kind(&content.kind)
                                            ),
                                        )
                                        .clicked()
                                    {
                                        self.authoring.composition_edit_selector = selector;
                                        self.authoring.composition_source_search.clear();
                                        self.authoring.composition_edit = None;
                                        self.authoring.composition_original_links = None;
                                        self.authoring.composition_link_candidate.clear();
                                        self.authoring.composition_link_search.clear();
                                        self.authoring.packagepack_engine_search.clear();
                                        self.authoring.packagepack_game_search.clear();
                                    }
                                }
                            });
                    });
                },
            );
            if ui
                .add_enabled(
                    !self.operation.running
                        && self.catalog.source_snapshot_current()
                        && !self.has_unsaved_workspace_edits()
                        && !composition_role_change_pending
                        && !self.authoring.composition_edit_selector.trim().is_empty(),
                    egui::Button::new("Edit included content"),
                )
                .on_disabled_hover_text(if !self.catalog.source_snapshot_current() {
                    "Refresh the project list before choosing a project from the previous results."
                } else {
                    "Save or discard project edits and resolve the pending content choice first."
                })
                .clicked()
            {
                self.load_source_composition();
            }
        });
        if !self.catalog.sources.is_empty() {
            ui.small(format!(
                "{source_match_count} project{}",
                if source_match_count == 1 { "" } else { "s" }
            ));
            if source_match_count == 0 {
                ui.weak("No projects match this search.");
            }
        }
        if composition_has_changes {
            ui.weak("Save or discard this draft before loading another project.");
        }
    }
}

fn is_additional_composition_dependency(
    content: &vapor_studio_core::SourceContent,
    composition: &vapor_studio_core::SourceComposition,
) -> bool {
    if content.id == composition.id {
        return false;
    }
    match (
        super::super::source_content_kind(&composition.kind),
        super::super::source_content_kind(&content.kind),
    ) {
        (Some(parent), Some(child)) => {
            vapor_studio_core::source_dependency_relationship(parent, child).is_some()
                && !(parent == vapor_core::ContentKind::Packagepack
                    && vapor_studio_core::source_dependency_choice_group(parent, child).is_some())
        }
        _ => false,
    }
}

fn source_content_matches_query(content: &vapor_studio_core::SourceContent, query: &str) -> bool {
    query.is_empty()
        || content.id.to_lowercase().contains(query)
        || content.kind.to_lowercase().contains(query)
        || content
            .version
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .contains(query)
}

fn packagepack_relationship_group(relationship: &str) -> Option<&'static str> {
    match relationship {
        "engine" | "enginepack" => Some("engine-choice"),
        "game" | "gamepack" => Some("game-choice"),
        _ => None,
    }
}

struct PackagepackRoleSlotSpec {
    group: &'static str,
    label: &'static str,
    empty_label: &'static str,
    optional: bool,
}

struct PackagepackRoleSlotContext<'a> {
    links: &'a mut Vec<SourceCompositionLink>,
    sources: &'a [vapor_studio_core::SourceContent],
    search: &'a mut String,
    pending_change: &'a mut Option<super::super::source_authoring::PendingCompositionRoleChange>,
}

fn draw_packagepack_role_slot(
    ui: &mut egui::Ui,
    spec: PackagepackRoleSlotSpec,
    context: PackagepackRoleSlotContext<'_>,
) {
    let PackagepackRoleSlotSpec {
        group,
        label,
        empty_label,
        optional,
    } = spec;
    let PackagepackRoleSlotContext {
        links,
        sources,
        search,
        pending_change,
    } = context;
    let packagepack = vapor_core::ContentKind::Packagepack;
    let choices = sources
        .iter()
        .filter_map(|content| {
            let kind = super::super::source_content_kind(&content.kind)?;
            if vapor_studio_core::source_dependency_choice_group(packagepack, kind) != Some(group) {
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
        choices
            .iter()
            .any(|(selector, _, _, _)| selector.split('@').next().unwrap_or(selector) == link.id)
    });
    let mut selected = current_link.map_or(PackagepackRoleSelection::Empty, |_| {
        current_choice.map_or_else(
            || {
                if current_id_is_local
                    && current_link.is_some_and(|link| {
                        link.version
                            .as_deref()
                            .is_none_or(|requirement| requirement == "*")
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
        (Some(_), PackagepackRoleSelection::Empty) => empty_label.to_owned(),
        (None, _) => empty_label.to_owned(),
    };

    let query = search.trim().to_lowercase();
    let matches_query =
        |(selector, kind, version, relationship): &(String, String, Option<String>, String)| {
            query.is_empty()
                || selector.to_lowercase().contains(&query)
                || kind.to_lowercase().contains(&query)
                || version
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&query)
                || relationship.to_lowercase().contains(&query)
        };
    let matching_choices = choices
        .iter()
        .filter(|choice| matches_query(choice))
        .count();

    ui.horizontal_wrapped(|ui| {
        ui.vertical(|ui| {
            ui.label(label);
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(search)
                        .desired_width(super::super::components::design::search_field_width(ui))
                        .hint_text(if optional {
                            "Find a Game"
                        } else {
                            "Find an Engine"
                        }),
                );
                if !search.is_empty() && ui.small_button("Clear").clicked() {
                    search.clear();
                }
            });
        });
        ui.vertical(|ui| {
            ui.small(format!(
                "{matching_choices} matching project{}",
                if matching_choices == 1 { "" } else { "s" }
            ));
            if matching_choices == 0 {
                ui.weak(if query.is_empty() {
                    "No compatible project is registered yet."
                } else {
                    "No compatible projects match this search."
                });
            }
        });
        ui.add_enabled_ui(pending_change.is_none(), |ui| {
            egui::ComboBox::from_id_salt(("packagepack-role", group))
                .width(ui.available_width().min(340.0))
                .selected_text(current_label)
                .show_ui(ui, |ui| {
                    if optional {
                        ui.selectable_value(
                            &mut selected,
                            PackagepackRoleSelection::Empty,
                            "No Game",
                        );
                    }
                    for (selector, kind, version, _) in
                        choices.iter().filter(|choice| matches_query(choice))
                    {
                        ui.selectable_value(
                            &mut selected,
                            PackagepackRoleSelection::Choice(selector.clone()),
                            packagepack_choice_label(selector, kind, version.as_deref()),
                        );
                    }
                });
        });
    });

    if selected != initial_selection {
        let previous = links
            .iter()
            .filter(|link| packagepack_relationship_group(&link.relationship) == Some(group))
            .cloned()
            .collect::<Vec<_>>();
        let proposed = if let PackagepackRoleSelection::Choice(selector) = selected
            && let Some((_, _, version, relationship)) =
                choices.iter().find(|choice| choice.0 == selector)
        {
            let id = selector.split('@').next().unwrap_or(&selector).to_owned();
            Some(SourceCompositionLink {
                id,
                relationship: relationship.clone(),
                optional: false,
                version: version.as_ref().map(|version| format!("={version}")),
                workshop_id: None,
                reason: None,
            })
        } else {
            None
        };
        if previous.is_empty() {
            links.retain(|link| packagepack_relationship_group(&link.relationship) != Some(group));
            if let Some(proposed) = proposed {
                links.push(proposed);
            }
        } else {
            *pending_change = Some(
                super::super::source_authoring::PendingCompositionRoleChange {
                    group,
                    previous,
                    proposed,
                },
            );
        }
    }

    if choices.is_empty() && !optional {
        ui.weak("No game engine project is available yet. Create one from a template or add an existing project folder.");
    }
}

fn packagepack_choice_label(selector: &str, kind: &str, version: Option<&str>) -> String {
    format!(
        "{} · {} · {}",
        selector.split('@').next().unwrap_or(selector),
        super::super::display_content_kind(kind),
        version.map_or_else(|| "unversioned".to_owned(), |value| format!("v{value}")),
    )
}
