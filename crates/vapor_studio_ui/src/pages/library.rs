use super::super::StudioApp;
use eframe::egui;
use vapor_studio_core::InstallationState;

impl StudioApp {
    pub(in crate::app) fn draw_library(&mut self, ui: &mut egui::Ui) {
        ui.heading("Compositions");
        ui.label("Profiles keep their own content choice, workspace, launch settings, and installed graph.");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.running && !self.workspace_editor_dirty,
                    egui::Button::new("Duplicate active profile"),
                )
                .on_disabled_hover_text(
                    "Save or discard editor changes before duplicating a profile.",
                )
                .clicked()
            {
                self.duplicate_active_profile();
            }
            ui.weak(
                "Copies the selected root and launch settings; installed artifacts remain shared.",
            );
        });

        for index in 0..self.profiles.len() {
            let profile = &self.profiles[index];
            let name = profile.name.clone();
            let root = profile
                .root_package
                .as_ref()
                .map(|identity| {
                    identity
                        .version
                        .as_ref()
                        .map(|version| format!("{}@{}", identity.id, version))
                        .unwrap_or_else(|| identity.id.clone())
                })
                .unwrap_or_else(|| "No Packagepack selected".to_owned());
            let selected = index == self.active_profile;
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !self.workspace_editor_dirty,
                        egui::Button::selectable(selected, name),
                    )
                    .on_disabled_hover_text(
                        "Save or discard editor changes before switching profiles.",
                    )
                    .clicked()
                {
                    self.select_profile(index);
                }
                ui.label(root);
            });
        }

        let Some(profile) = self.profiles.get(self.active_profile).cloned() else {
            return;
        };
        ui.separator();
        ui.heading(&profile.name);
        if let Some(root) = &profile.root_package {
            ui.label(format!("Root: {}", content_identity(root)));
        } else {
            ui.weak("Choose a Packagepack from Discover to create a launchable composition.");
        }

        if let Some(root) = &profile.root_package {
            let selector = content_identity(root);
            let editable_source = self.contents.iter().any(|content| {
                content.id == root.id
                    && content.kind == vapor_core::ContentKind::Packagepack.as_str()
            });
            if ui
                .add_enabled(
                    !self.running && !self.workspace_editor_dirty && editable_source,
                    egui::Button::new("Edit source graph"),
                )
                .on_disabled_hover_text("Only a Packagepack authored in the active workspace can be edited here.")
                .clicked()
            {
                self.composition_edit_selector = selector.clone();
                self.composition_edit = None;
                self.composition_original_links = None;
                self.section = super::super::Section::Compose;
                self.load_source_composition();
            }
            if ui
                .add_enabled(
                    !self.running && !self.workspace_editor_dirty,
                    egui::Button::new("Preview dependency plan"),
                )
                .on_disabled_hover_text(
                    "Save or reload the open file first; resolution reads manifests from disk.",
                )
                .clicked()
            {
                self.preview_selected_composition();
            }
            let preview = self
                .composition_preview
                .as_ref()
                .filter(|preview| preview.selector == selector)
                .cloned();
            if let Some(preview) = preview {
                if let Some(published_file_id) =
                    super::super::components::composition_preview::draw_composition_preview(
                        ui, &preview,
                    )
                {
                    self.workshop_id = published_file_id;
                    self.section = super::super::Section::Discover;
                    self.download_workshop_item();
                }
            }
        }

        if let Some(graph) = &profile.resolved_graph {
            ui.group(|ui| {
                ui.label(format!(
                    "Resolved composition · {} content nodes",
                    graph.nodes.len()
                ));
                if let InstallationState::Installed {
                    graph_fingerprint: Some(fingerprint),
                    ..
                } = &profile.installation
                {
                    ui.small(format!("Resolved graph fingerprint: {fingerprint}"));
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Engine: {}",
                        content_identity(&graph.effective_engine)
                    ));
                    ui.label(format!(
                        "Game: {}",
                        graph
                            .effective_game
                            .as_ref()
                            .map(content_identity)
                            .unwrap_or_else(|| "None".to_owned())
                    ));
                });
                egui::CollapsingHeader::new("Resolved content")
                    .default_open(true)
                    .show(ui, |ui| {
                        for node in &graph.nodes {
                            ui.horizontal(|ui| {
                                ui.label(node.kind.as_str());
                                ui.label(content_identity(&node.content));
                                ui.weak(format!(
                                    "Payload {}",
                                    node.fingerprint.as_deref().unwrap_or("not fingerprinted")
                                ));
                                if let Some(source) = &node.source {
                                    ui.weak(format!("Installed from {source}"));
                                }
                                if !node.dependencies.is_empty() {
                                    ui.weak(format!("{} dependencies", node.dependencies.len()));
                                }
                            });
                        }
                    });
            });
        } else if profile.root_package.is_some() {
            ui.weak("Install this composition to resolve and record its exact dependency graph.");
        }

        ui.separator();
        ui.label("Launch arguments");
        ui.weak("Each row is passed as one argument to the installed Engine.");
        let active_profile = self.active_profile;
        let can_edit = !self.running;
        let mut remove_argument = None;
        for index in 0..self.profiles[active_profile].launch.arguments.len() {
            ui.horizontal(|ui| {
                ui.label(format!("{}", index + 1));
                ui.add_enabled(
                    can_edit,
                    egui::TextEdit::singleline(
                        &mut self.profiles[active_profile].launch.arguments[index],
                    )
                    .desired_width(320.0),
                );
                if ui
                    .add_enabled(can_edit, egui::Button::new("Remove"))
                    .clicked()
                {
                    remove_argument = Some(index);
                }
            });
        }
        if let Some(index) = remove_argument {
            self.profiles[active_profile].launch.arguments.remove(index);
        }
        if ui
            .add_enabled(can_edit, egui::Button::new("Add argument"))
            .clicked()
        {
            self.profiles[active_profile]
                .launch
                .arguments
                .push(String::new());
        }

        ui.horizontal_wrapped(|ui| {
            let has_root = profile.root_package.is_some();
            let has_source = profile.workspace.is_some();
            let root_plan_node = profile.root_package.as_ref().and_then(|root| {
                let selector = content_identity(root);
                self.composition_preview.as_ref()
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
            if ui.add_enabled(!self.running && !self.workspace_editor_dirty && has_root && has_source, egui::Button::new("Validate source"))
                .on_disabled_hover_text("Source validation needs this profile to be associated with a workspace. Cached or seeded compositions validate during install.")
                .clicked() { self.validate_selected_source(); }
            if ui.add_enabled(!self.running && !self.workspace_editor_dirty && has_root && preview_ready && !installed && !root_installed && root_has_refresh_source, egui::Button::new("Install composition"))
                .on_disabled_hover_text("Preview a ready plan with an installable workspace or cache root. An already installed root needs verification, playback, or an update.")
                .clicked() { self.install_selected(); }
            if ui.add_enabled(!self.running && !self.workspace_editor_dirty && has_root && installed && preview_ready && root_has_refresh_source, egui::Button::new("Update composition"))
                .on_disabled_hover_text("Update uses the ready plan's current workspace or cached sources.")
                .clicked() { self.update_selected(); }
            if ui.add_enabled(!self.running && !self.workspace_editor_dirty && has_root && has_source && root_is_workspace_source, egui::Button::new("Build and play source"))
                .on_disabled_hover_text("The selected Packagepack must resolve from this workspace before source build/play is available.")
                .clicked() { self.run_selected(); }
            if ui.add_enabled(!self.running && has_root && (installed || root_installed), egui::Button::new("Play installed")).clicked() { self.run_installed_selected(); }
            if ui.add_enabled(!self.running && has_root && (installed || root_installed), egui::Button::new("Verify graph")).clicked() { self.verify_selected_installation(); }
            if ui.add_enabled(!self.running && !self.workspace_editor_dirty && has_root && preview_ready && root_has_refresh_source && needs_repair, egui::Button::new("Repair"))
                .on_disabled_hover_text("Repair needs a ready workspace or cached source to replace damaged installed files.")
                .clicked() { self.repair_selected_installation(); }
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
                !self.running && !self.workspace_editor_dirty,
                egui::Button::new("Discover content"),
            )
            .on_disabled_hover_text(
                "Save or reload the open file first; discovery reads saved workspace manifests.",
            )
            .clicked()
        {
            self.section = super::super::Section::Discover;
            self.discover();
        }
    }
}

fn content_identity(identity: &vapor_studio_core::ContentIdentity) -> String {
    identity
        .version
        .as_ref()
        .map(|version| format!("{}@{}", identity.id, version))
        .unwrap_or_else(|| identity.id.clone())
}
