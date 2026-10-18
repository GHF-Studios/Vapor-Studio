use super::super::{Section, StudioApp};
use super::design::{StatusTone, status_text};
use eframe::egui;
use std::path::PathBuf;

enum ContentDetailAction {
    StageWorkshop(String),
    ReviewPublish,
    ReviewComposition,
    CreateComposition(vapor_client_application::ContentReference, Option<PathBuf>),
    PrepareAddToComposition(vapor_client_application::ContentReference, String),
    SelectComposition(usize),
    InstallComposition,
    PlayComposition,
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
                    let lifecycle_actions = self
                        .catalog
                        .service_item_actions
                        .get(&(content.id.clone(), content.version.clone()))
                        .cloned()
                        .unwrap_or_default();
                    let catalog_snapshot_current = self.catalog.local_catalog_current();
                    if lifecycle_actions.contains(&vapor_client_protocol::CatalogItemAction::Play)
                    {
                        if ui
                            .add_enabled(!self.operation.running, egui::Button::new("Play"))
                            .clicked()
                        {
                            action = Some(ContentDetailAction::PlayComposition);
                        }
                    } else if lifecycle_actions
                        .contains(&vapor_client_protocol::CatalogItemAction::Install)
                    {
                        if ui
                            .add_enabled(
                                !self.operation.running && !self.has_unsaved_workspace_edits(),
                                egui::Button::new("Install"),
                            )
                            .clicked()
                        {
                            action = Some(ContentDetailAction::InstallComposition);
                        }
                    } else if lifecycle_actions
                        .contains(&vapor_client_protocol::CatalogItemAction::Review)
                    {
                        if ui
                            .add_enabled(
                                !self.operation.running && !self.has_unsaved_workspace_edits(),
                                egui::Button::new("Review installation"),
                            )
                            .clicked()
                        {
                            action = Some(ContentDetailAction::ReviewComposition);
                        }
                    } else if lifecycle_actions.contains(
                        &vapor_client_protocol::CatalogItemAction::SelectComposition,
                    ) {
                        let target = self.composition.items.iter().position(|composition| {
                            composition.root_package.as_ref().is_some_and(|root| {
                                root.id == content.id && root.version == content.version
                            })
                        });
                        if let Some(target) = target
                            && ui
                                .add_enabled(
                                    !self.operation.running
                                        && !self.has_unsaved_context_switch_edits(),
                                    egui::Button::new("Open composition"),
                                )
                                .clicked()
                        {
                            action = Some(ContentDetailAction::SelectComposition(target));
                        }
                    } else if selected_for_composition {
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
                    } else if lifecycle_actions.contains(
                        &vapor_client_protocol::CatalogItemAction::CreateComposition,
                    ) && ui
                        .add_enabled(
                            !self.operation.running
                                && !self.has_unsaved_workspace_edits()
                                && !create_draft_blocks_assignment
                                && catalog_snapshot_current
                                && content.is_unambiguous(),
                            egui::Button::new("Create composition"),
                        )
                        .clicked()
                    {
                        action = Some(ContentDetailAction::CreateComposition(
                            content.identity(),
                            content_workspace,
                        ));
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
                        action = Some(ContentDetailAction::CreateComposition(
                            content.identity(),
                            content_workspace,
                        ));
                    }
                } else if self
                    .catalog
                    .service_item_actions
                    .get(&(content.id.clone(), content.version.clone()))
                    .is_some_and(|actions| {
                        actions.contains(&vapor_client_protocol::CatalogItemAction::AddToComposition)
                    })
                {
                    let eligible = self.eligible_compositions_for_content(content);
                    if !eligible.is_empty() {
                        let selected = self
                            .catalog
                            .add_to_composition_target
                            .as_ref()
                            .and_then(|id| {
                                eligible.iter().find(|index| self.composition.items[**index].id == *id)
                            })
                            .copied()
                            .or_else(|| {
                                eligible.iter().copied().find(|index| {
                                    *index == self.composition.active_index
                                })
                            })
                            .unwrap_or(eligible[0]);
                        self.catalog.add_to_composition_target =
                            Some(self.composition.items[selected].id.clone());
                        ui.label("Add this project to a composition from the same project folder.");
                        egui::ComboBox::from_id_salt(("add-content-target", &content.id))
                            .selected_text(&self.composition.items[selected].name)
                            .show_ui(ui, |ui| {
                                for index in &eligible {
                                    let composition = &self.composition.items[*index];
                                    if ui
                                        .selectable_label(*index == selected, &composition.name)
                                        .clicked()
                                    {
                                        self.catalog.add_to_composition_target =
                                            Some(composition.id.clone());
                                    }
                                }
                            });
                        if ui
                            .add_enabled(
                                !self.operation.running
                                    && !self.has_unsaved_context_switch_edits()
                                    && self.catalog.local_catalog_current()
                                    && self.catalog.source_snapshot_current(),
                                egui::Button::new("Add to composition"),
                            )
                            .on_disabled_hover_text(
                                "Refresh the project list and save or discard open edits before changing the included content.",
                            )
                            .clicked()
                        {
                            let target = self.catalog.add_to_composition_target.clone().unwrap_or_default();
                            action = Some(ContentDetailAction::PrepareAddToComposition(
                                content.identity(),
                                target,
                            ));
                        }
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
            Some(ContentDetailAction::CreateComposition(identity, workspace)) => {
                self.create_composition_from_content(identity, workspace);
            }
            Some(ContentDetailAction::PrepareAddToComposition(identity, composition_id)) => {
                self.prepare_add_to_composition(identity, &composition_id);
            }
            Some(ContentDetailAction::SelectComposition(index)) => {
                self.select_composition(index);
                self.activate_navigation_section(Section::Compositions);
            }
            Some(ContentDetailAction::InstallComposition) => {
                self.activate_navigation_section(Section::Compositions);
                self.install_selected();
            }
            Some(ContentDetailAction::ReviewComposition) => {
                self.activate_navigation_section(Section::Compositions);
                self.preview_selected_composition();
            }
            Some(ContentDetailAction::PlayComposition) => {
                self.run_installed_selected();
            }
            None => {}
        }
    }

    fn eligible_compositions_for_content(
        &self,
        content: &vapor_client_application::CatalogContent,
    ) -> Vec<usize> {
        let workspace = self.workspace.root.trim();
        if workspace.is_empty()
            || !content.locations.iter().any(|location| {
                location.kind == vapor_client_application::CatalogLocationKind::Workspace
                    && location.root.to_string_lossy() == workspace
            })
            || !self.catalog.sources.iter().any(|source| {
                super::super::source_content_selector_matches(
                    source,
                    &super::super::catalog_content_selector(content),
                )
            })
        {
            return Vec::new();
        }
        let Some(child_kind) = super::super::source_content_kind(&content.kind) else {
            return Vec::new();
        };
        self.composition
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, composition)| {
                let root = composition.root_package.as_ref()?;
                let association = composition.workspace.as_ref()?;
                if association.root.to_string_lossy() != workspace {
                    return None;
                }
                let root_content = self.catalog.items.iter().find(|candidate| {
                    candidate.id == root.id
                        && candidate.version == root.version
                        && candidate.kind == vapor_core::ContentKind::Packagepack.as_str()
                        && candidate.locations.iter().any(|location| {
                            location.kind == vapor_client_application::CatalogLocationKind::Workspace
                                && location.root.to_string_lossy() == workspace
                        })
                })?;
                let parent_kind = super::super::source_content_kind(&root_content.kind)?;
                vapor_client_application::source_dependency_allowed(parent_kind, child_kind)
                    .then_some(index)
            })
            .collect()
    }

    fn prepare_add_to_composition(
        &mut self,
        identity: vapor_client_application::ContentReference,
        composition_id: &str,
    ) {
        if self.has_unsaved_context_switch_edits() {
            self.status = "Save or discard open edits before changing a composition.".to_owned();
            return;
        }
        let Some(index) = self
            .composition
            .items
            .iter()
            .position(|composition| composition.id == composition_id)
        else {
            self.status = "That composition is no longer available.".to_owned();
            return;
        };
        let Some(root) = self.composition.items[index].root_package.clone() else {
            self.status = "Choose a playable composition before adding content.".to_owned();
            return;
        };
        let Some(association) = self.composition.items[index].workspace.as_ref() else {
            self.status = "This composition has no editable project folder. Open or create a project-backed composition first.".to_owned();
            return;
        };
        if association.root.to_string_lossy() != self.workspace.root.trim() {
            self.status = "Open this composition's project folder before adding its content.".to_owned();
            return;
        }
        if index != self.composition.active_index {
            self.select_composition(index);
            if self.composition.active_index != index {
                return;
            }
        }
        self.authoring.composition_edit_selector = root.version.as_deref().map_or_else(
            || root.id.clone(),
            |version| format!("{}@{version}", root.id),
        );
        self.authoring.composition_edit = None;
        self.authoring.composition_original_links = None;
        self.authoring.composition_link_candidate = identity.version.as_deref().map_or_else(
            || identity.id.clone(),
            |version| format!("{}@{version}", identity.id),
        );
        self.authoring.pending_composition_add = Some(identity.clone());
        self.authoring.composition_link_search.clear();
        self.activate_navigation_section(Section::Compose);
        if !self.operation.running && self.catalog.source_snapshot_current() {
            self.load_source_composition();
        } else {
            self.status = format!(
                "Open the included-content editor for {} after the project list refreshes; {} will be added to the unsaved project draft.",
                self.composition.items[index].name,
                identity.id,
            );
        }
    }
}
