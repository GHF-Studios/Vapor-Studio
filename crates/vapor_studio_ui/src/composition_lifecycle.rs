use super::{StudioApp, content_selector};
use super::operation_protocol::OperationResult;
use std::path::PathBuf;
use vapor_studio_core::{ContentIdentity, InstalledArtifactRecord, OperationKind};

pub(super) struct ProfileCompositionState {
    preview: Option<vapor_studio_core::CompositionPreview>,
    repreview_after_catalog_refresh: bool,
}

impl Default for ProfileCompositionState {
    fn default() -> Self {
        Self {
            preview: None,
            repreview_after_catalog_refresh: false,
        }
    }
}

impl ProfileCompositionState {
    pub(super) fn preview(&self) -> Option<&vapor_studio_core::CompositionPreview> {
        self.preview.as_ref()
    }

    pub(super) fn set_preview(&mut self, preview: vapor_studio_core::CompositionPreview) {
        self.preview = Some(preview);
    }

    pub(super) fn invalidate_preview(&mut self) {
        self.preview = None;
    }

    pub(super) fn clear_context(&mut self) {
        self.invalidate_preview();
        self.repreview_after_catalog_refresh = false;
    }

    pub(super) fn request_repreview_after_catalog_refresh(&mut self) {
        self.repreview_after_catalog_refresh = true;
    }

    pub(super) fn take_repreview_request(&mut self) -> bool {
        std::mem::take(&mut self.repreview_after_catalog_refresh)
    }
}

impl StudioApp {
    pub(super) fn apply_composition_preview(
        &mut self,
        preview: vapor_studio_core::CompositionPreview,
    ) {
        self.status = if preview.ready() {
            format!(
                "Resolved {} content nodes. This composition is ready to install.",
                preview.nodes.len()
            )
        } else {
            format!(
                "Resolved {} available nodes with {} blocker(s). Review the plan before installing.",
                preview.nodes.len(),
                preview.blockers.len()
            )
        };
        self.profile_composition.set_preview(preview);
    }

    pub(super) fn apply_installed_composition(
        &mut self,
        composition: vapor_studio_core::InstalledComposition,
        status: &str,
    ) {
        self.profile_composition.invalidate_preview();
        let graph_fingerprint = vapor_studio_core::fingerprint_resolved_graph(&composition.graph);
        self.profile.items[self.profile.active_index].resolved_graph = Some(composition.graph);
        self.profile.items[self.profile.active_index].installation =
            vapor_studio_core::InstallationState::Installed {
                artifacts: installed_records(composition.artifacts),
                graph_fingerprint,
            };
        self.status = status.to_owned();
    }

    pub(super) fn apply_installation_check(
        &mut self,
        check: vapor_studio_core::InstallationCheck,
    ) {
        let graph_fingerprint = check
            .graph
            .as_ref()
            .and_then(vapor_studio_core::fingerprint_resolved_graph);
        if let Some(graph) = check.graph {
            self.profile.items[self.profile.active_index].resolved_graph = Some(graph);
        }
        let checked = check.detail.len();
        let detail = check.detail.join("; ");
        let artifacts = installed_records(check.artifacts);
        if check.ok {
            self.profile.items[self.profile.active_index].installation =
                vapor_studio_core::InstallationState::Installed {
                    artifacts,
                    graph_fingerprint,
                };
            self.status = format!("Installed composition verified. {checked} graph nodes checked.");
        } else {
            self.profile.items[self.profile.active_index].installation =
                vapor_studio_core::InstallationState::NeedsRepair {
                    detail: detail.clone(),
                    artifacts,
                };
            self.status = format!("Installed composition needs repair: {detail}");
        }
    }

    pub(super) fn apply_verification_error(&mut self, error: String) {
        let artifacts = match self.profile.items[self.profile.active_index].installation.clone() {
            vapor_studio_core::InstallationState::Installed { artifacts, .. }
            | vapor_studio_core::InstallationState::NeedsRepair { artifacts, .. } => artifacts,
            _ => Vec::new(),
        };
        self.profile.items[self.profile.active_index].installation =
            vapor_studio_core::InstallationState::NeedsRepair {
                detail: error.clone(),
                artifacts,
            };
        self.status = format!("Could not verify the full composition: {error}");
    }

    pub(super) fn apply_composition_action_error(&mut self, error: String) {
        self.profile.items[self.profile.active_index].installation =
            vapor_studio_core::InstallationState::Partial {
                detail: error.clone(),
            };
        self.status = error;
    }

    pub(super) fn install_selected(&mut self) {
        if !self.require_saved_workspace("install this workspace composition") {
            return;
        }
        let Some(profile) = self.profile.items.get(self.profile.active_index).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let workspace = profile.workspace.map(|workspace| workspace.root);
        let executable = match std::env::current_exe() {
            Ok(executable) => executable,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = root_package
            .version
            .as_deref()
            .map(|version| format!("{}@{}", root_package.id, version))
            .unwrap_or_else(|| root_package.id.clone());
        self.spawn(OperationKind::Install, move |_context| {
            OperationResult::Install(vapor_studio_core::install_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
            ))
        });
        self.status = "Installing the selected composition and dependencies…".to_owned();
    }

    pub(super) fn update_selected(&mut self) {
        if !self.require_saved_workspace("update this workspace composition") {
            return;
        }
        let Some(profile) = self.profile.items.get(self.profile.active_index).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let workspace = profile.workspace.map(|workspace| workspace.root);
        let executable = match std::env::current_exe() {
            Ok(executable) => executable,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        self.spawn(OperationKind::Update, move |_context| {
            OperationResult::Update(vapor_studio_core::update_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
            ))
        });
        self.status = "Updating the selected composition through Vapor Shell…".to_owned();
    }

    pub(super) fn preview_selected_composition(&mut self) {
        if !self.require_saved_workspace("resolve this workspace composition") {
            return;
        }
        let Some(profile) = self.profile.items.get(self.profile.active_index).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let selector = content_selector(&root_package);
        let workspace = profile.workspace.map(|workspace| workspace.root);
        self.preview_composition(selector, workspace, "Resolving this profile against workspace, cache, and installed content…");
    }

    pub(super) fn preview_source_composition(&mut self, selector: String) {
        if !self.require_open_workspace("preview this source composition") {
            return;
        }
        let workspace = Some(PathBuf::from(self.workspace.root.trim()));
        self.preview_composition(selector, workspace, "Resolving this source composition against workspace, cache, and installed content…");
    }

    pub(super) fn preview_composition(
        &mut self,
        selector: String,
        workspace: Option<PathBuf>,
        status: &str,
    ) {
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.profile_composition.invalidate_preview();
        self.spawn(OperationKind::Resolve, move |_context| {
            OperationResult::CompositionPreview(vapor_studio_core::preview_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
            ))
        });
        self.status = status.to_owned();
    }

    pub(super) fn verify_selected_installation(&mut self) {
        let Some(root_package) = self
            .profile
            .items
            .get(self.profile.active_index)
            .and_then(|profile| profile.root_package.clone())
        else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        let selectors = self.profile.items[self.profile.active_index]
            .resolved_graph
            .as_ref()
            .map(|graph| {
                graph
                    .nodes
                    .iter()
                    .map(|node| content_selector(&node.content))
                    .collect()
            })
            .unwrap_or_else(|| vec![content_selector(&root_package)]);
        self.spawn(OperationKind::Verify, move |_context| {
            OperationResult::Verify(vapor_studio_core::verify_installation(
                &executable,
                &content_selector(&root_package),
                &selectors,
            ))
        });
        self.status = "Verifying installed composition…".to_owned();
    }

    pub(super) fn repair_selected_installation(&mut self) {
        if !self.require_saved_workspace("repair this workspace composition") {
            return;
        }
        let Some(root_package) = self
            .profile
            .items
            .get(self.profile.active_index)
            .and_then(|profile| profile.root_package.clone())
        else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        let workspace = self.profile.items[self.profile.active_index]
            .workspace
            .as_ref()
            .map(|workspace| workspace.root.clone());
        let selector = content_selector(&root_package);
        let graph_selectors = self.profile.items[self.profile.active_index]
            .resolved_graph
            .as_ref()
            .map(|graph| {
                graph
                    .nodes
                    .iter()
                    .map(|node| content_selector(&node.content))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.spawn(OperationKind::Repair, move |_context| {
            OperationResult::Repair(vapor_studio_core::repair_profile_composition(
                &executable,
                workspace.as_deref(),
                &selector,
                &graph_selectors,
            ))
        });
        self.status = "Repairing installed composition…".to_owned();
    }

    pub(super) fn run_installed_selected(&mut self) {
        let Some(profile) = self.profile.items.get(self.profile.active_index).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        let workspace = profile.workspace.map(|workspace| workspace.root);
        let arguments = profile.launch.arguments;
        self.spawn(OperationKind::RunInstalled, move |_context| {
            OperationResult::Message(vapor_studio_core::run_installed_profile(
                &executable,
                workspace.as_deref(),
                &selector,
                &arguments,
            ))
        });
        self.status = "Launching the installed composition for this profile…".to_owned();
    }

    pub(super) fn launch_default_profile(&mut self) {
        if self.profile.items[self.profile.active_index].root_package.is_none() {
            let executable = match std::env::current_exe() {
                Ok(value) => value,
                Err(error) => {
                    self.status = format!("Could not locate the Studio executable: {error}");
                    return;
                }
            };
            match vapor_studio_core::default_packagepack_selector(&executable, "default") {
                Ok(id) => {
                    self.profile.items[self.profile.active_index].root_package =
                        Some(ContentIdentity { id, version: None })
                }
                Err(error) => {
                    self.status = format!("This app has no default playable composition: {error}");
                    return;
                }
            }
        }
        self.run_installed_selected();
    }

    pub(super) fn run_selected(&mut self) {
        if !self.require_open_workspace("build and play this workspace composition") {
            return;
        }
        let Some(profile) = self.profile.items.get(self.profile.active_index).cloned() else {
            return;
        };
        let Some(root_package) = profile.root_package else {
            return;
        };
        let Some(workspace) = profile.workspace.map(|workspace| workspace.root) else {
            return;
        };
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let selector = content_selector(&root_package);
        let arguments = profile.launch.arguments;
        self.spawn(OperationKind::Run, move |_context| {
            OperationResult::RunProfile(vapor_studio_core::run_source_profile(
                &executable,
                &workspace,
                &selector,
                &arguments,
            ))
        });
        self.status =
            "Installing source changes and launching this profile through Vapor Shell…".to_owned();
    }

}

pub(super) fn installed_records(
    artifacts: Vec<vapor_studio_core::InstalledArtifact>,
) -> Vec<InstalledArtifactRecord> {
    artifacts
        .into_iter()
        .map(|artifact| InstalledArtifactRecord {
            id: artifact.id,
            version: artifact.version,
            root: artifact.root,
            fingerprint: artifact.fingerprint,
        })
        .collect()
}
