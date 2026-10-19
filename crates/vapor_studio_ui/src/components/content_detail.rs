use super::super::{Section, StudioApp};
use super::design::{StatusTone, status_text};
use eframe::egui;
use std::path::PathBuf;

enum ContentDetailAction {
    StageWorkshop(String),
    ForkWorkshop { selector: String, name: String },
    ReviewPublish,
    ReviewComposition,
    CreateComposition(vapor_client_application::ContentReference, Option<PathBuf>),
    PrepareAddToComposition(vapor_client_application::ContentReference, String),
    SelectComposition(usize),
    InstallComposition,
    PlayComposition,
    SetAutoInstall { content_id: String, enabled: bool },
}

impl StudioApp {
    #[allow(clippy::collapsible_if)]
    pub(in crate::app) fn draw_content_detail(&mut self, ui: &mut egui::Ui) {
        let selected = self.catalog.selected.clone();
        if self.catalog.fork_identity != selected {
            self.catalog.fork_identity = selected.clone();
            let base = selected
                .as_deref()
                .unwrap_or_default()
                .split('@')
                .next()
                .unwrap_or_default()
                .rsplit('/')
                .next()
                .unwrap_or("content")
                .to_ascii_lowercase();
            let slug = base
                .chars()
                .map(|character| {
                    if character.is_ascii_lowercase() || character.is_ascii_digit() {
                        character
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
                .split('-')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("-");
            self.catalog.fork_name =
                format!("{}-copy", if slug.is_empty() { "content" } else { &slug });
        }
        let mut action = None;
        if let Some(identity) = self.catalog.selected.as_ref() {
            if let Some(content) =
                self.catalog.items.iter().find(|content| {
                    super::super::catalog_content_selector_matches(content, identity)
                })
            {
                ui.heading(super::super::display_catalog_name(content));
                ui.label(super::super::display_content_kind(&content.kind));
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
                    "Requirements · {}",
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
                egui::CollapsingHeader::new("Technical details")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.small(format!("Content ID: {}", content.id));
                        ui.small(format!(
                            "Version: {}",
                            content.version.as_deref().unwrap_or("not specified")
                        ));
                        if let Some(published_file_id) = content.published_file_id.as_deref() {
                            ui.small(format!("Workshop item: {published_file_id}"));
                        }
                    });
                let is_cached_library_item = content.locations.iter().any(|location| {
                    location.kind == vapor_client_application::CatalogLocationKind::Cache
                })
                    && vapor_client_domain::can_auto_install_library_item(
                        &content.kind,
                        content.published_file_id.is_some(),
                        true,
                        content.is_unambiguous(),
                    );
                if is_cached_library_item {
                    let referenced = self.composition.items.iter().any(|composition| {
                        composition.resolved_graph.as_ref().is_some_and(|graph| {
                            graph.nodes.iter().any(|node| node.content.id == content.id)
                        })
                    });
                    let mut enabled = self.catalog.library_policies.iter().any(|policy| {
                        policy.content_id.as_str() == content.id && policy.auto_install
                    });
                    let mut policy_changed = false;
                    ui.add_enabled_ui(referenced || enabled, |ui| {
                        policy_changed = ui
                            .checkbox(&mut enabled, "Automatically update this content")
                            .on_hover_text("Follow compatible Workshop versions for compositions that use this item, and remove its local download when no composition needs it.")
                            .changed();
                    });
                    if !referenced && !enabled {
                        ui.small(
                            "Add this item to a composition before enabling automatic updates.",
                        );
                    }
                    if policy_changed {
                        action = Some(ContentDetailAction::SetAutoInstall {
                            content_id: content.id.clone(),
                            enabled,
                        });
                    }
                }
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
                let acquired_runtime_content = !is_authored_here
                    && content.kind != vapor_client_protocol::ContentKind::Packagepack.as_str()
                    && content.kind != vapor_client_protocol::ContentKind::Crate.as_str()
                    && content.locations.iter().any(|location| {
                        location.kind == vapor_client_application::CatalogLocationKind::Cache
                    });
                if acquired_runtime_content {
                    let can_fork = !self.operation.running
                        && !self.workspace.root.trim().is_empty()
                        && !self.has_unsaved_workspace_edits()
                        && self.catalog.local_catalog_current();
                    ui.group(|ui| {
                        ui.strong("Make your own copy");
                        ui.label("Fork this item into the open workspace with a new local identity. The Workshop package must include its editable source files.");
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Project name");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.catalog.fork_name)
                                    .desired_width(190.0)
                                    .hint_text("my-content-copy"),
                            );
                        });
                        let name = self.catalog.fork_name.trim();
                        let valid_name = !name.is_empty()
                            && name.bytes().all(|byte| {
                                byte.is_ascii_lowercase()
                                    || byte.is_ascii_digit()
                                    || byte == b'-'
                            })
                            && name.bytes().next().is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                            && name.bytes().last().is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
                        if ui
                            .add_enabled(
                                valid_name
                                    && can_fork,
                                egui::Button::new("Fork into workspace"),
                            )
                            .on_disabled_hover_text("Open a workspace, choose a valid project name, and save or discard edits before forking.")
                            .clicked()
                        {
                            action = Some(ContentDetailAction::ForkWorkshop {
                                selector: content.version.as_deref().map_or_else(
                                    || content.id.clone(),
                                    |version| format!("{}@{version}", content.id),
                                ),
                                name: name.to_owned(),
                            });
                        }
                    });
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
                if content.kind == vapor_client_protocol::ContentKind::Packagepack.as_str() {
                    let content_workspace = content
                        .locations
                        .iter()
                        .any(|location| {
                            location.kind
                                == vapor_client_application::CatalogLocationKind::Workspace
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
                    } else if ui
                        .add_enabled(
                            !self.operation.running
                                && !self.has_unsaved_workspace_edits()
                                && !create_draft_blocks_assignment
                                && catalog_snapshot_current
                                && content.is_unambiguous(),
                            egui::Button::new(if lifecycle_actions.contains(
                                &vapor_client_protocol::CatalogItemAction::CreateComposition,
                            ) {
                                "Create composition"
                            } else {
                                "Use as the current composition"
                            }),
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
                        actions
                            .contains(&vapor_client_protocol::CatalogItemAction::AddToComposition)
                    })
                {
                    let eligible = self.eligible_compositions_for_content(content);
                    if !eligible.is_empty() {
                        let selected = self
                            .catalog
                            .add_to_composition_target
                            .as_ref()
                            .and_then(|id| {
                                eligible
                                    .iter()
                                    .find(|index| self.composition.items[**index].id == *id)
                            })
                            .copied()
                            .or_else(|| {
                                eligible
                                    .iter()
                                    .copied()
                                    .find(|index| *index == self.composition.active_index)
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
            Some(ContentDetailAction::ForkWorkshop { selector, name }) => {
                let workspace_root = self.workspace.root.trim().to_owned();
                self.start_service_operation(
                    vapor_client_protocol::ClientAction::Authoring(
                        vapor_client_protocol::AuthoringAction::ForkContent {
                            workspace_root,
                            content_id: selector,
                            name,
                        },
                    ),
                    None,
                    vapor_client_application::OperationKind::ForkContent,
                    vapor_client_application::OperationPhase::Building,
                    "Forking source into your workspace…",
                );
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
            Some(ContentDetailAction::SetAutoInstall {
                content_id,
                enabled,
            }) => {
                self.submit_service_action(vapor_client_protocol::ClientAction::Library(
                    vapor_client_protocol::LibraryAction::SetAutoInstall {
                        content_id,
                        enabled,
                    },
                ));
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
                        && candidate.kind
                            == vapor_client_protocol::ContentKind::Packagepack.as_str()
                        && candidate.locations.iter().any(|location| {
                            location.kind
                                == vapor_client_application::CatalogLocationKind::Workspace
                                && location.root.to_string_lossy() == workspace
                        })
                })?;
                let parent_kind = super::super::source_content_kind(&root_content.kind)?;
                vapor_client_domain::dependency_kind_allowed(parent_kind, child_kind)
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
            self.status =
                "Open this composition's project folder before adding its content.".to_owned();
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
                self.composition.items[index].name, identity.id,
            );
        }
    }
}
