use super::super::StudioApp;
use super::super::content_selector;
use eframe::egui;
use vapor_studio_core::{
    ContentIdentity, InstallationState, ResolvedCompositionGraph, ResolvedNodeIdentity,
};

#[derive(Clone, Copy)]
enum ProfilePrimaryAction {
    PlayInstalled,
    Repair,
    Verify,
    Update,
    ReviewPlan,
    BuildAndPlay,
    Install,
}

fn identity_label(identity: &ContentIdentity) -> String {
    identity.version.as_deref().map_or_else(
        || identity.id.clone(),
        |version| format!("{}@{version}", identity.id),
    )
}

fn canonical_dependencies(node: &ResolvedNodeIdentity) -> Vec<(String, ContentIdentity)> {
    let mut dependencies = node
        .dependencies
        .iter()
        .map(|dependency| (dependency.binding.clone(), dependency.content.clone()))
        .collect::<Vec<_>>();
    dependencies.sort();
    dependencies
}

fn graph_difference_lines(
    saved: &ResolvedCompositionGraph,
    installed: &ResolvedCompositionGraph,
) -> Vec<String> {
    let mut differences = Vec::new();
    if saved.root != installed.root {
        differences.push(format!(
            "Selected content changed: {} → {}",
            identity_label(&saved.root),
            identity_label(&installed.root),
        ));
    }
    if saved.effective_engine != installed.effective_engine {
        differences.push(format!(
            "Engine changed: {} → {}",
            identity_label(&saved.effective_engine),
            identity_label(&installed.effective_engine),
        ));
    }
    if saved.effective_game != installed.effective_game {
        differences.push(format!(
            "Game changed: {} → {}",
            saved
                .effective_game
                .as_ref()
                .map(identity_label)
                .unwrap_or_else(|| "none".to_owned()),
            installed
                .effective_game
                .as_ref()
                .map(identity_label)
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
                identity_label(&expected.content),
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
                identity_label(&expected.content),
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
                identity_label(&actual.content),
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
    pub(super) fn draw_profile_installation_panel(
        &mut self,
        ui: &mut eframe::egui::Ui,
        profile: &vapor_studio_core::CompositionProfile,
    ) {
        let has_root = profile.root_package.is_some();
        let has_source = profile.workspace.is_some();
        let root_plan_node = profile.root_package.as_ref().and_then(|root| {
            let selector = content_selector(root);
            self.profile_composition.preview()
                .filter(|preview| preview.selector == selector && preview.ready())
                .and_then(|preview| preview.nodes.last())
        });
        let preview_ready = root_plan_node.is_some();
        let root_installed = root_plan_node.is_some_and(|node| node.already_installed);
        let root_has_refresh_source = root_plan_node.is_some_and(|node| {
            node.source != vapor_studio_core::CompositionPreviewSource::Installed
        });
        let root_is_workspace_source = root_plan_node.is_some_and(|node| {
            node.source == vapor_studio_core::CompositionPreviewSource::Workspace
        });
        let installed = matches!(&profile.installation, InstallationState::Installed { .. });
        let needs_repair = matches!(&profile.installation,
            InstallationState::NeedsRepair { .. } | InstallationState::Partial { .. });
        let out_of_date = matches!(&profile.installation, InstallationState::OutOfDate { .. });
        let primary_action = if !has_root {
            None
        } else if installed && !needs_repair && !out_of_date {
            Some(("Play installed", ProfilePrimaryAction::PlayInstalled))
        } else if needs_repair {
            Some((
                if preview_ready && root_has_refresh_source {
                    "Repair composition"
                } else {
                    "Verify installation"
                },
                if preview_ready && root_has_refresh_source {
                    ProfilePrimaryAction::Repair
                } else {
                    ProfilePrimaryAction::Verify
                },
            ))
        } else if out_of_date {
            Some((
                if preview_ready && root_has_refresh_source {
                    "Update composition"
                } else {
                    "Review update plan"
                },
                if preview_ready && root_has_refresh_source {
                    ProfilePrimaryAction::Update
                } else {
                    ProfilePrimaryAction::ReviewPlan
                },
            ))
        } else if root_is_workspace_source && has_source {
            Some(("Build and play source", ProfilePrimaryAction::BuildAndPlay))
        } else if preview_ready && !root_installed && root_has_refresh_source {
            Some(("Install composition", ProfilePrimaryAction::Install))
        } else {
            Some(("Preview dependency plan", ProfilePrimaryAction::ReviewPlan))
        };
        if let Some((label, action)) = primary_action {
            let can_start = !self.operation.running
                && (matches!(action, ProfilePrimaryAction::PlayInstalled)
                    || !self.has_unsaved_workspace_edits());
            let mut clicked = false;
            ui.add_enabled_ui(can_start, |ui| {
                clicked = super::super::components::design::primary_button(ui, label).clicked();
            });
            if clicked {
                match action {
                    ProfilePrimaryAction::PlayInstalled => self.run_installed_selected(),
                    ProfilePrimaryAction::Repair => self.repair_selected_installation(),
                    ProfilePrimaryAction::Verify => self.verify_selected_installation(),
                    ProfilePrimaryAction::Update => self.update_selected(),
                    ProfilePrimaryAction::ReviewPlan => self.preview_selected_composition(),
                    ProfilePrimaryAction::BuildAndPlay => self.run_selected(),
                    ProfilePrimaryAction::Install => self.install_selected(),
                }
            }
        }
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!self.operation.running && !self.has_unsaved_workspace_edits() && has_root && has_source, egui::Button::new("Validate source"))
                .on_disabled_hover_text("Source validation needs this profile to be associated with a workspace. Cached or seeded compositions validate during install.")
                .clicked() { self.validate_selected_source(); }
            if ui.add_enabled(!self.operation.running && has_root && (installed || root_installed), egui::Button::new("Verify installed content")).clicked() { self.verify_selected_installation(); }
        });

        ui.label("Installation");
        match &profile.installation {
            InstallationState::Unknown => {
                ui.weak("Not checked")
                    .on_hover_text("Studio has not inspected this profile's installed content.");
            }
            InstallationState::NotInstalled => {
                ui.weak("Not installed");
            }
            InstallationState::Installed { artifacts, .. } => {
                ui.label(format!(
                    "{} exact content versions installed",
                    artifacts.len()
                ));
                egui::CollapsingHeader::new("Installed file locations")
                    .show(ui, |ui| {
                        for artifact in artifacts {
                            ui.small(format!(
                                "{}{} · {}",
                                artifact.id,
                                artifact
                                    .version
                                    .as_ref()
                                    .map(|version| format!("@{version}"))
                                    .unwrap_or_default(),
                                artifact.root.display()
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
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Installed content differs from this profile's saved setup",
                );
                ui.label(detail);
                if !verified {
                    ui.label("Verify the installed files before saving these changes to the profile.");
                }
                if let Some(saved_graph) = &profile.resolved_graph {
                    egui::CollapsingHeader::new("Review changes")
                        .default_open(true)
                        .show(ui, |ui| {
                            for difference in graph_difference_lines(saved_graph, actual_graph) {
                                ui.small(difference);
                            }
                        });
                } else {
                    ui.weak("This profile has no saved content selection yet.");
                }
                ui.small(format!(
                    "Installed setup: {} content items · engine {}",
                    actual_graph.nodes.len(),
                    actual_graph.effective_engine.id,
                ));
                egui::CollapsingHeader::new("Installed content details")
                    .show(ui, |ui| {
                        for node in &actual_graph.nodes {
                            ui.small(format!(
                                "{} · {}{} · {} · {}",
                                node.kind,
                                node.content.id,
                                node.content
                                    .version
                                    .as_deref()
                                    .map(|version| format!("@{version}"))
                                    .unwrap_or_default(),
                                node.fingerprint.as_deref().unwrap_or("unfingerprinted"),
                                node.source.as_deref().unwrap_or("source unknown"),
                            ));
                        }
                        ui.small(format!("{} installed artifacts observed", artifacts.len()));
                    });
                if ui
                    .add_enabled(
                        !self.operation.running && *verified,
                        egui::Button::new("Save installed setup to profile"),
                    )
                    .on_hover_text(if *verified {
                        "Update this profile to match the verified content currently installed."
                    } else {
                        "Verify the installed files before saving this setup to the profile."
                    })
                    .clicked()
                {
                    self.adopt_installed_profile_graph();
                }
            }
            InstallationState::Partial { detail } => {
                ui.colored_label(egui::Color32::YELLOW, "Installation needs attention");
                ui.label(detail);
            }
            InstallationState::NeedsRepair { detail, artifacts } => {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("{} installed versions need repair", artifacts.len()),
                );
                ui.label(detail);
            }
        }

        ui.separator();
        if ui
            .add_enabled(
                !self.operation.running && !self.has_unsaved_workspace_edits(),
                egui::Button::new("Discover content"),
            )
            .on_disabled_hover_text(
                "Save or discard workspace file and structured metadata drafts first; discovery reads saved manifests.",
            )
            .clicked()
        {
            self.section = super::super::Section::Discover;
            self.discover();
        }
    }
}

#[cfg(test)]
mod graph_difference_tests {
    use super::graph_difference_lines;
    use vapor_studio_core::{
        ContentIdentity, ResolvedCompositionGraph, ResolvedDependencyIdentity, ResolvedNodeIdentity,
    };

    fn identity(id: &str, version: &str) -> ContentIdentity {
        ContentIdentity {
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
                        },
                        ResolvedDependencyIdentity {
                            binding: "game".to_owned(),
                            content: identity("org.gameplay", "3.0.0"),
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
            graph_difference_lines(&saved, &installed),
            ["No content differences found; verify the installed files again."],
        );

        installed.nodes[1].fingerprint = Some("sha256:changed".to_owned());
        installed.nodes[1].source = Some("cache".to_owned());
        installed.nodes[1].dependencies[1].content = identity("org.other-engine", "2.0.0");
        installed.nodes[2].content = identity("org.other-engine", "2.0.0");
        installed.effective_engine = identity("org.other-engine", "2.0.0");
        let differences = graph_difference_lines(&saved, &installed);
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
