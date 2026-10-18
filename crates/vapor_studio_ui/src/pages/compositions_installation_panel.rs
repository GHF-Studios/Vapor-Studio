use super::super::StudioApp;
use super::super::components::design::{StatusTone, status_text};
use super::super::content_selector;
use eframe::egui;
use vapor_client_application::{
    ContentReference, InstallationState, ResolvedCompositionGraph, ResolvedNodeIdentity,
};

#[derive(Clone, Copy)]
pub(super) enum CompositionInstallationAction {
    Play,
    Repair,
    Verify,
    Update,
    ReviewPlan,
    BuildAndPlay,
    Install,
    ValidateSource,
    VerifyInstalled,
    AdoptInstalledGraph,
    DiscoverContent,
}

fn identity_label(
    identity: &ContentReference,
    catalog: &[vapor_client_application::CatalogContent],
) -> String {
    super::super::display_catalog_identity(&identity.id, identity.version.as_deref(), catalog)
}

fn canonical_dependencies(node: &ResolvedNodeIdentity) -> Vec<(&str, &ContentReference)> {
    let mut dependencies = node
        .dependencies
        .iter()
        .map(|dependency| (dependency.binding.as_str(), &dependency.content))
        .collect::<Vec<_>>();
    dependencies.sort();
    dependencies
}

fn graph_difference_lines(
    saved: &ResolvedCompositionGraph,
    installed: &ResolvedCompositionGraph,
    catalog: &[vapor_client_application::CatalogContent],
) -> Vec<String> {
    let mut differences = Vec::new();
    if saved.root != installed.root {
        differences.push(format!(
            "Selected content changed: {} → {}",
            identity_label(&saved.root, catalog),
            identity_label(&installed.root, catalog),
        ));
    }
    if saved.effective_engine != installed.effective_engine {
        differences.push(format!(
            "Engine changed: {} → {}",
            identity_label(&saved.effective_engine, catalog),
            identity_label(&installed.effective_engine, catalog),
        ));
    }
    if saved.effective_game != installed.effective_game {
        differences.push(format!(
            "Game changed: {} → {}",
            saved
                .effective_game
                .as_ref()
                .map(|identity| identity_label(identity, catalog))
                .unwrap_or_else(|| "none".to_owned()),
            installed
                .effective_game
                .as_ref()
                .map(|identity| identity_label(identity, catalog))
                .unwrap_or_else(|| "none".to_owned()),
        ));
    }
    for expected in &saved.nodes {
        let Some(actual) = installed
            .nodes
            .iter()
            .find(|node| node.content == expected.content)
        else {
            differences.push(format!(
                "No longer installed: {} · {}",
                expected.kind,
                identity_label(&expected.content, catalog),
            ));
            continue;
        };
        let mut changes = Vec::new();
        if expected.kind != actual.kind {
            changes.push("content type".to_owned());
        }
        if expected.fingerprint != actual.fingerprint {
            changes.push("file contents".to_owned());
        }
        if expected.source != actual.source {
            changes.push("source location".to_owned());
        }
        if canonical_dependencies(expected) != canonical_dependencies(actual) {
            changes.push("dependencies".to_owned());
        }
        if !changes.is_empty() {
            differences.push(format!(
                "Changed {} · {}",
                identity_label(&expected.content, catalog),
                changes.join(", "),
            ));
        }
    }
    for actual in &installed.nodes {
        if !saved
            .nodes
            .iter()
            .any(|expected| expected.content == actual.content)
        {
            differences.push(format!(
                "Also installed: {} · {}",
                actual.kind,
                identity_label(&actual.content, catalog),
            ));
        }
    }
    if differences.is_empty() {
        differences
            .push("No content differences found; verify the installed files again.".to_owned());
    }
    differences
}

impl StudioApp {
    pub(super) fn draw_composition_installation_panel(
        &self,
        ui: &mut eframe::egui::Ui,
        composition: &vapor_client_application::Composition,
    ) -> Option<CompositionInstallationAction> {
        let mut action_to_run = None;
        let has_root = composition.root_package.is_some();
        let has_source = composition.workspace.is_some();
        let selected_preview = composition.root_package.as_ref().and_then(|root| {
            let selector = content_selector(root);
            self.composition_preview
                .preview()
                .filter(|preview| preview.selector == selector)
        });
        let root_plan_node = composition.root_package.as_ref().and_then(|root| {
            selected_preview
                .and_then(|preview| preview.nodes.iter().find(|node| node.id == root.id))
        });
        let plan_needs_install = selected_preview.is_some_and(|preview| {
            preview.nodes.iter().any(|node| {
                !node.already_installed
                    && node.source != vapor_client_application::CompositionPreviewSource::Installed
            })
        });
        let plan_has_refresh_source = selected_preview.is_some_and(|preview| {
            preview
                .nodes
                .iter()
                .any(|node| node.source != vapor_client_application::CompositionPreviewSource::Installed)
        });
        let root_installed = root_plan_node.is_some_and(|node| node.already_installed);
        let root_is_workspace_source = composition.root_package.as_ref().is_some_and(|root| {
            root_plan_node.is_some_and(|node| {
                node.source == vapor_client_application::CompositionPreviewSource::Workspace
            }) || self.catalog.sources.iter().any(|content| {
                content.id == root.id
                    && content.kind == vapor_core::ContentKind::Packagepack.as_str()
            })
        });
        let installed = matches!(
            &composition.installation,
            InstallationState::Installed { .. }
        );
        let needs_repair = matches!(
            &composition.installation,
            InstallationState::NeedsRepair { .. } | InstallationState::Partial { .. }
        );
        let out_of_date = matches!(
            &composition.installation,
            InstallationState::OutOfDate { .. }
        );
        if has_root {
            let (label, action) = if needs_repair {
                if plan_has_refresh_source || has_source {
                    ("Repair composition", CompositionInstallationAction::Repair)
                } else {
                    ("Verify installation", CompositionInstallationAction::Verify)
                }
            } else if out_of_date {
                if plan_has_refresh_source || has_source {
                    ("Update composition", CompositionInstallationAction::Update)
                } else {
                    ("Review update", CompositionInstallationAction::ReviewPlan)
                }
            } else if installed {
                ("Launch", CompositionInstallationAction::Play)
            } else if root_is_workspace_source && has_source {
                (
                    "Build and launch project",
                    CompositionInstallationAction::BuildAndPlay,
                )
            } else {
                (
                    "Install composition",
                    CompositionInstallationAction::Install,
                )
            };
            let needs_saved_project = match action {
                CompositionInstallationAction::Play | CompositionInstallationAction::Verify => {
                    root_is_workspace_source
                }
                CompositionInstallationAction::VerifyInstalled
                | CompositionInstallationAction::AdoptInstalledGraph
                | CompositionInstallationAction::ValidateSource
                | CompositionInstallationAction::DiscoverContent => false,
                _ => true,
            };
            let project_is_dirty = needs_saved_project && self.has_unsaved_workspace_edits();
            let mut clicked = false;
            ui.add_enabled_ui(!self.operation.running && !project_is_dirty, |ui| {
                clicked = super::super::components::design::primary_button(ui, label).clicked();
            })
            .response
            .on_disabled_hover_text(if self.operation.running {
                "Wait for the current operation to finish."
            } else {
                "Save or discard project edits before changing or running this setup."
            });
            if clicked {
                action_to_run = Some(action);
            }
        }
        if has_root {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !self.operation.running
                            && !self.has_unsaved_workspace_edits()
                            && has_source,
                        egui::Button::new("Check project"),
                    )
                    .on_disabled_hover_text(
                        "This check is for a project folder open in Studio. Downloaded content is checked during install.",
                    )
                    .clicked()
                {
                    action_to_run = Some(CompositionInstallationAction::ValidateSource);
                }
                if ui
                    .add_enabled(
                        !self.operation.running && (installed || root_installed),
                        egui::Button::new("Check installed files"),
                    )
                    .clicked()
                {
                    action_to_run = Some(CompositionInstallationAction::VerifyInstalled);
                }
            });

            ui.label("Available on this device");
            match &composition.installation {
                InstallationState::Unknown if root_installed && plan_needs_install => {
                    status_text(ui, "Composition incomplete", StatusTone::Caution);
                    ui.weak(
                        "The Packagepack is present, but required engine or game content still needs installation.",
                    );
                }
                InstallationState::Unknown if root_installed => {
                    status_text(ui, "Composition content not checked", StatusTone::Neutral);
                    ui.weak(
                        "Install or verify the engine and required content before launching this composition.",
                    );
                }
                InstallationState::Unknown => {
                    ui.weak("Not checked").on_hover_text(
                        "Studio has not inspected this composition's installed content.",
                    );
                }
                InstallationState::NotInstalled if root_installed && plan_needs_install => {
                    status_text(ui, "Composition incomplete", StatusTone::Caution);
                    ui.weak(
                        "The Packagepack is present, but required engine or game content still needs installation.",
                    );
                }
                InstallationState::NotInstalled if root_installed => {
                    status_text(ui, "Composition content not verified", StatusTone::Neutral);
                }
                InstallationState::NotInstalled => {
                    ui.weak("Not installed");
                }
                InstallationState::Installed { artifacts, .. } => {
                    ui.label(format!("{} content items installed", artifacts.len()));
                    egui::CollapsingHeader::new("Installed content").show(ui, |ui| {
                        for artifact in artifacts {
                            ui.small(super::super::display_catalog_identity(
                                &artifact.id,
                                artifact.version.as_deref(),
                                &self.catalog.items,
                            ));
                        }
                    });
                }
                InstallationState::OutOfDate {
                    detail,
                    actual_graph,
                    artifacts,
                    verified,
                } => {
                    status_text(
                        ui,
                        "Installed content differs from this composition's saved setup",
                        StatusTone::Caution,
                    );
                    ui.weak("Review the changed content below before updating this composition.");
                    egui::CollapsingHeader::new("Technical details")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.small(detail);
                        });
                    if !verified {
                        ui.label(
                            "Verify the installed files before saving these changes to the composition.",
                        );
                    }
                    if let Some(saved_graph) = &composition.resolved_graph {
                        egui::CollapsingHeader::new("Review changes")
                            .default_open(true)
                            .show(ui, |ui| {
                                for difference in graph_difference_lines(
                                    saved_graph,
                                    actual_graph,
                                    &self.catalog.items,
                                )
                                {
                                    ui.small(difference);
                                }
                            });
                    } else {
                        ui.weak("This composition has no saved content selection yet.");
                    }
                    ui.small(format!(
                        "Installed setup: {} content items · engine {}",
                        actual_graph.nodes.len(),
                        super::super::display_catalog_identity(
                            &actual_graph.effective_engine.id,
                            actual_graph.effective_engine.version.as_deref(),
                            &self.catalog.items,
                        ),
                    ));
                    egui::CollapsingHeader::new("Content details").show(ui, |ui| {
                        for node in &actual_graph.nodes {
                            ui.small(format!(
                                "{} · {}",
                                super::super::display_content_kind(&node.kind),
                                super::super::display_catalog_identity(
                                    &node.content.id,
                                    node.content.version.as_deref(),
                                    &self.catalog.items,
                                ),
                            ));
                        }
                        ui.small(format!("{} content items installed", artifacts.len()));
                    });
                    if ui
                        .add_enabled(
                            !self.operation.running && *verified,
                            egui::Button::new("Update from installed content"),
                        )
                        .on_hover_text(if *verified {
                            "Update this composition to match the verified content currently installed."
                        } else {
                            "Verify the installed files before saving this setup to the composition."
                        })
                        .clicked()
                    {
                        action_to_run = Some(CompositionInstallationAction::AdoptInstalledGraph);
                    }
                }
                InstallationState::Partial { detail } => {
                    status_text(ui, "Installation needs attention", StatusTone::Caution);
                    ui.weak("Some required content could not be checked or installed.");
                    egui::CollapsingHeader::new("Technical details")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.small(detail);
                        });
                }
                InstallationState::NeedsRepair { detail, artifacts } => {
                    status_text(
                        ui,
                        &format!("{} installed versions need repair", artifacts.len()),
                        StatusTone::Caution,
                    );
                    ui.weak("Repair the installed content before launching this composition.");
                    egui::CollapsingHeader::new("Technical details")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.small(detail);
                        });
                }
            }
        }

        ui.separator();
        if ui
            .add_enabled(
                !self.operation.running && !self.has_unsaved_workspace_edits(),
                egui::Button::new("Discover content"),
            )
            .on_disabled_hover_text(
                "Save or discard your project edits first; Studio reads saved project details for this check.",
            )
            .clicked()
        {
            action_to_run = Some(CompositionInstallationAction::DiscoverContent);
        }
        action_to_run
    }
}

#[cfg(test)]
mod graph_difference_tests {
    use super::graph_difference_lines;
    use vapor_client_application::{
        ContentReference, ResolvedCompositionGraph, ResolvedDependencyIdentity, ResolvedNodeIdentity,
    };

    fn identity(id: &str, version: &str) -> ContentReference {
        ContentReference {
            id: id.to_owned(),
            version: Some(version.to_owned()),
        }
    }

    fn graph() -> ResolvedCompositionGraph {
        let engine = identity("org.engine", "2.0.0");
        let game = identity("org.gameplay", "3.0.0");
        let root = identity("org.game", "1.0.0");
        ResolvedCompositionGraph {
            root: root.clone(),
            effective_engine: engine.clone(),
            effective_game: Some(game.clone()),
            nodes: vec![
                ResolvedNodeIdentity {
                    content: engine,
                    kind: "Engine".to_owned(),
                    fingerprint: Some("sha256:engine".to_owned()),
                    source: Some("workshop:42".to_owned()),
                    dependencies: Vec::new(),
                },
                ResolvedNodeIdentity {
                    content: root,
                    kind: "Packagepack".to_owned(),
                    fingerprint: Some("sha256:root".to_owned()),
                    source: Some("workspace:/game".to_owned()),
                    dependencies: vec![
                        ResolvedDependencyIdentity {
                            binding: "engine".to_owned(),
                            content: identity("org.engine", "2.0.0"),
                            version_requirement: None,
                        },
                        ResolvedDependencyIdentity {
                            binding: "game".to_owned(),
                            content: identity("org.gameplay", "3.0.0"),
                            version_requirement: None,
                        },
                    ],
                },
                ResolvedNodeIdentity {
                    content: game,
                    kind: "Game".to_owned(),
                    fingerprint: Some("sha256:game".to_owned()),
                    source: Some("cache".to_owned()),
                    dependencies: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn graph_review_reports_changes_and_ignores_record_order() {
        let saved = graph();
        let mut installed = saved.clone();
        installed.nodes.reverse();
        installed.nodes[1].dependencies.reverse();
        assert_eq!(
            graph_difference_lines(&saved, &installed, &[]),
            ["No content differences found; verify the installed files again."],
        );

        installed.nodes[1].fingerprint = Some("sha256:changed".to_owned());
        installed.nodes[1].source = Some("cache".to_owned());
        installed.nodes[1].dependencies[1].content = identity("org.other-engine", "2.0.0");
        installed.nodes[2].content = identity("org.other-engine", "2.0.0");
        installed.effective_engine = identity("org.other-engine", "2.0.0");
        let differences = graph_difference_lines(&saved, &installed, &[]);
        assert!(
            differences
                .iter()
                .any(|line| line.contains("file contents, source location, dependencies"))
        );
        assert!(
            differences
                .iter()
                .any(|line| line.contains("No longer installed"))
        );
        assert!(
            differences
                .iter()
                .any(|line| line.contains("Also installed"))
        );
    }
}
