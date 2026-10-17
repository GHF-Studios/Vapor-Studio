use super::super::StudioApp;
use super::super::components::design::{StatusTone, status_text};
use super::super::content_selector;
use eframe::egui;
use vapor_studio_core::{
    ContentIdentity, InstallationState, ResolvedCompositionGraph, ResolvedNodeIdentity,
};

#[derive(Clone, Copy)]
pub(super) enum ProfileInstallationAction {
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

fn identity_label(identity: &ContentIdentity) -> String {
    identity.version.as_deref().map_or_else(
        || identity.id.clone(),
        |version| format!("{}@{version}", identity.id),
    )
}

fn canonical_dependencies<'a>(
    node: &'a ResolvedNodeIdentity,
) -> Vec<(&'a str, &'a ContentIdentity)> {
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
        &self,
        ui: &mut eframe::egui::Ui,
        profile: &vapor_studio_core::CompositionProfile,
    ) -> Option<ProfileInstallationAction> {
        let mut action_to_run = None;
        let has_root = profile.root_package.is_some();
        let has_source = profile.workspace.is_some();
        let selected_preview = profile.root_package.as_ref().and_then(|root| {
            let selector = content_selector(root);
            self.profile_composition
                .preview()
                .filter(|preview| preview.selector == selector)
        });
        let root_plan_node = profile.root_package.as_ref().and_then(|root| {
            selected_preview.and_then(|preview| {
                preview.nodes.iter().find(|node| node.id == root.id)
            })
        });
        let plan_needs_install = selected_preview.is_some_and(|preview| {
            preview.nodes.iter().any(|node| {
                !node.already_installed
                    && node.source != vapor_studio_core::CompositionPreviewSource::Installed
            })
        });
        let plan_has_refresh_source = selected_preview.is_some_and(|preview| {
            preview
                .nodes
                .iter()
                .any(|node| node.source != vapor_studio_core::CompositionPreviewSource::Installed)
        });
        let root_installed = root_plan_node.is_some_and(|node| node.already_installed);
        let root_is_workspace_source = profile.root_package.as_ref().is_some_and(|root| {
            root_plan_node.is_some_and(|node| {
                node.source == vapor_studio_core::CompositionPreviewSource::Workspace
            }) || self.catalog.sources.iter().any(|content| {
                content.id == root.id
                    && content.kind == vapor_core::ContentKind::Packagepack.as_str()
            })
        });
        let installed = matches!(&profile.installation, InstallationState::Installed { .. });
        let needs_repair = matches!(
            &profile.installation,
            InstallationState::NeedsRepair { .. } | InstallationState::Partial { .. }
        );
        let out_of_date = matches!(&profile.installation, InstallationState::OutOfDate { .. });
        if has_root {
            let (label, action) = if needs_repair {
                if plan_has_refresh_source || has_source {
                    ("Repair game setup", ProfileInstallationAction::Repair)
                } else {
                    ("Verify installation", ProfileInstallationAction::Verify)
                }
            } else if out_of_date {
                if plan_has_refresh_source || has_source {
                    ("Update game setup", ProfileInstallationAction::Update)
                } else {
                    ("Review update", ProfileInstallationAction::ReviewPlan)
                }
            } else if installed {
                ("Launch", ProfileInstallationAction::Play)
            } else if root_is_workspace_source && has_source {
                ("Build and launch project", ProfileInstallationAction::BuildAndPlay)
            } else {
                ("Install game setup", ProfileInstallationAction::Install)
            };
            let needs_saved_project = match action {
                ProfileInstallationAction::Play | ProfileInstallationAction::Verify => {
                    root_is_workspace_source
                }
                ProfileInstallationAction::VerifyInstalled
                | ProfileInstallationAction::AdoptInstalledGraph
                | ProfileInstallationAction::ValidateSource
                | ProfileInstallationAction::DiscoverContent => false,
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
                    action_to_run = Some(ProfileInstallationAction::ValidateSource);
                }
                if ui
                    .add_enabled(
                        !self.operation.running && (installed || root_installed),
                        egui::Button::new("Check installed files"),
                    )
                    .clicked()
                {
                    action_to_run = Some(ProfileInstallationAction::VerifyInstalled);
                }
            });

            ui.label("Available on this device");
            match &profile.installation {
                InstallationState::Unknown if root_installed && plan_needs_install => {
                    status_text(ui, "Game setup incomplete", StatusTone::Caution);
                    ui.weak(
                        "The Packagepack is present, but required engine or game content still needs installation.",
                    );
                }
                InstallationState::Unknown if root_installed => {
                    status_text(
                        ui,
                        "Packagepack present · setup not checked",
                        StatusTone::Neutral,
                    );
                    ui.weak(
                        "Install or verify the full engine and game setup before relying on this profile.",
                    );
                }
                InstallationState::Unknown => {
                    ui.weak("Not checked")
                        .on_hover_text("Studio has not inspected this profile's installed content.");
                }
                InstallationState::NotInstalled if root_installed && plan_needs_install => {
                    status_text(ui, "Game setup incomplete", StatusTone::Caution);
                    ui.weak(
                        "The Packagepack is present, but required engine or game content still needs installation.",
                    );
                }
                InstallationState::NotInstalled if root_installed => {
                    status_text(
                        ui,
                        "Packagepack present · setup not verified",
                        StatusTone::Neutral,
                    );
                }
                InstallationState::NotInstalled => {
                    ui.weak("Not installed");
                }
                InstallationState::Installed { artifacts, .. } => {
                    ui.label(format!(
                        "{} content items installed",
                        artifacts.len()
                    ));
                    egui::CollapsingHeader::new("Installed file locations").show(ui, |ui| {
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
                    status_text(
                        ui,
                        "Installed content differs from this profile's saved setup",
                        StatusTone::Caution,
                    );
                    ui.label(detail);
                    if !verified {
                        ui.label(
                            "Verify the installed files before saving these changes to the profile.",
                        );
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
                    egui::CollapsingHeader::new("Installed content details").show(ui, |ui| {
                        for node in &actual_graph.nodes {
                            ui.small(format!(
                                "{} · {}{} · {} · {}",
                                super::super::display_content_kind(&node.kind),
                                node.content.id,
                                node.content
                                    .version
                                    .as_deref()
                                    .map(|version| format!("@{version}"))
                                    .unwrap_or_default(),
                                node.fingerprint.as_deref().unwrap_or("unfingerprinted"),
                                node.source.as_deref().unwrap_or("origin not recorded"),
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
                        action_to_run = Some(ProfileInstallationAction::AdoptInstalledGraph);
                    }
                }
                InstallationState::Partial { detail } => {
                    status_text(ui, "Installation needs attention", StatusTone::Caution);
                    ui.label(detail);
                }
                InstallationState::NeedsRepair { detail, artifacts } => {
                    status_text(
                        ui,
                        &format!("{} installed versions need repair", artifacts.len()),
                        StatusTone::Caution,
                    );
                    ui.label(detail);
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
            action_to_run = Some(ProfileInstallationAction::DiscoverContent);
        }
        action_to_run
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
