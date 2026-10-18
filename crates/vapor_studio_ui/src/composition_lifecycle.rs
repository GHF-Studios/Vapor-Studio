use super::StudioApp;
use std::path::PathBuf;
use vapor_studio_core::OperationKind;
use vapor_studio_protocol::StudioAction;

pub(super) enum RepreviewAfterCatalogRefresh {
    ActiveProfile,
    Source {
        selector: String,
        workspace_root: PathBuf,
    },
}

#[derive(Default)]
pub(super) struct ProfileCompositionState {
    preview: Option<vapor_studio_core::CompositionPreview>,
    repreview_after_catalog_refresh: Option<RepreviewAfterCatalogRefresh>,
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
        self.repreview_after_catalog_refresh = None;
    }

    pub(super) fn request_repreview_after_catalog_refresh(&mut self) {
        self.repreview_after_catalog_refresh = Some(RepreviewAfterCatalogRefresh::ActiveProfile);
    }

    pub(super) fn request_source_repreview_after_catalog_refresh(
        &mut self,
        selector: String,
        workspace_root: PathBuf,
    ) {
        self.repreview_after_catalog_refresh = Some(RepreviewAfterCatalogRefresh::Source {
            selector,
            workspace_root,
        });
    }

    pub(super) fn take_repreview_request(&mut self) -> Option<RepreviewAfterCatalogRefresh> {
        self.repreview_after_catalog_refresh.take()
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
                "Found {} available content items; {} need attention. Review the plan before installing.",
                preview.nodes.len(),
                preview.blockers.len()
            )
        };
        self.profile_composition.set_preview(preview);
    }

    pub(super) fn adopt_installed_profile_graph(&mut self) {
        let Some(profile) = self.profile.items.get(self.profile.active_index) else {
            return;
        };
        if !matches!(
            &profile.installation,
            vapor_studio_core::InstallationState::OutOfDate { verified: true, .. }
        ) {
            return;
        }
        let profile_id = profile.id.clone();
        match self.apply_service_action(StudioAction::Profile(
            vapor_studio_protocol::ProfileAction::AdoptVerifiedGraph { profile_id },
        )) {
            Ok(()) => self.status = "Saved the verified installed setup to this profile.".into(),
            Err(error) => self.status = error,
        }
    }

    pub(super) fn install_selected(&mut self) {
        if !self.require_saved_workspace("install this workspace composition") {
            return;
        }
        let Some(profile_id) = self
            .profile
            .items
            .get(self.profile.active_index)
            .map(|p| p.id.clone())
        else {
            return;
        };
        self.start_service_composition_operation(
            StudioAction::Composition(vapor_studio_protocol::CompositionAction::Install {
                profile_id,
            }),
            OperationKind::Install,
            vapor_studio_core::OperationPhase::Acquiring,
            "Installing the selected composition and dependencies…",
        );
    }

    pub(super) fn update_selected(&mut self) {
        if !self.require_saved_workspace("update this workspace composition") {
            return;
        }
        let Some(profile_id) = self
            .profile
            .items
            .get(self.profile.active_index)
            .map(|p| p.id.clone())
        else {
            return;
        };
        self.start_service_composition_operation(
            StudioAction::Composition(vapor_studio_protocol::CompositionAction::Update {
                profile_id,
            }),
            OperationKind::Update,
            vapor_studio_core::OperationPhase::Acquiring,
            "Updating the selected composition through Vapor Shell…",
        );
    }

    pub(super) fn preview_selected_composition(&mut self) {
        if !self.require_saved_workspace("resolve this workspace composition") {
            return;
        }
        let Some(profile_id) = self
            .profile
            .items
            .get(self.profile.active_index)
            .map(|p| p.id.clone())
        else {
            return;
        };
        self.start_service_composition_operation(
            StudioAction::Composition(vapor_studio_protocol::CompositionAction::Preview {
                profile_id,
                selector: None,
            }),
            OperationKind::Resolve,
            vapor_studio_core::OperationPhase::Resolving,
            "Resolving this profile against workspace, cache, and installed content…",
        );
    }

    pub(super) fn preview_source_composition(&mut self, selector: String) {
        if !self.require_open_workspace("preview this source composition") {
            return;
        }
        let workspace = Some(PathBuf::from(self.workspace.root.trim()));
        self.preview_composition(
            selector,
            workspace,
            "Resolving this source composition against workspace, cache, and installed content…",
        );
    }

    pub(super) fn preview_composition(
        &mut self,
        selector: String,
        _workspace: Option<PathBuf>,
        status: &str,
    ) {
        let Some(profile_id) = self
            .profile
            .items
            .get(self.profile.active_index)
            .map(|p| p.id.clone())
        else {
            return;
        };
        self.profile_composition.invalidate_preview();
        self.start_service_composition_operation(
            StudioAction::Composition(vapor_studio_protocol::CompositionAction::Preview {
                profile_id,
                selector: Some(selector),
            }),
            OperationKind::Resolve,
            vapor_studio_core::OperationPhase::Resolving,
            status,
        );
    }

    pub(super) fn verify_selected_installation(&mut self) {
        let Some(profile_id) = self
            .profile
            .items
            .get(self.profile.active_index)
            .map(|profile| profile.id.clone())
        else {
            return;
        };
        self.start_service_composition_operation(
            StudioAction::Composition(vapor_studio_protocol::CompositionAction::Verify {
                profile_id,
            }),
            OperationKind::Verify,
            vapor_studio_core::OperationPhase::Verifying,
            "Verifying installed composition…",
        );
    }

    pub(super) fn repair_selected_installation(&mut self) {
        if !self.require_saved_workspace("repair this workspace composition") {
            return;
        }
        let Some(profile_id) = self
            .profile
            .items
            .get(self.profile.active_index)
            .map(|profile| profile.id.clone())
        else {
            return;
        };
        self.start_service_composition_operation(
            StudioAction::Composition(vapor_studio_protocol::CompositionAction::Repair {
                profile_id,
            }),
            OperationKind::Repair,
            vapor_studio_core::OperationPhase::Acquiring,
            "Repairing installed composition…",
        );
    }

    pub(super) fn run_installed_selected(&mut self) {
        let Some(profile) = self.profile.items.get(self.profile.active_index).cloned() else {
            return;
        };
        if matches!(
            &profile.installation,
            vapor_studio_core::InstallationState::OutOfDate { .. }
        ) {
            self.status = "Review or update this profile's game setup before launching.".to_owned();
            return;
        }
        if profile.root_package.is_none() {
            return;
        }
        self.start_service_composition_operation(
            StudioAction::Composition(vapor_studio_protocol::CompositionAction::Launch { profile_id: profile.id }),
            OperationKind::RunInstalled,
            vapor_studio_core::OperationPhase::Running,
            "Preparing game setup. Steam must be open and signed in if Workshop items need downloading…",
        );
    }

    pub(super) fn ensure_default_profile_root(&mut self) -> bool {
        if self.profile.items[self.profile.active_index]
            .root_package
            .is_some()
        {
            return true;
        }
        let profile_id = self.profile.items[self.profile.active_index].id.clone();
        match self.apply_service_action(StudioAction::Profile(
            vapor_studio_protocol::ProfileAction::ResolveDefaultRoot { profile_id },
        )) {
            Ok(()) => true,
            Err(error) => {
                self.status = error;
                false
            }
        }
    }

    pub(super) fn launch_default_profile(&mut self) {
        if self.ensure_default_profile_root() {
            self.run_installed_selected();
        }
    }

    pub(super) fn run_selected(&mut self) {
        if !self.require_open_workspace("build and play this workspace composition") {
            return;
        }
        let Some(profile) = self.profile.items.get(self.profile.active_index).cloned() else {
            return;
        };
        if profile.root_package.is_none() || profile.workspace.is_none() {
            return;
        }
        self.start_service_composition_operation(
            StudioAction::Composition(vapor_studio_protocol::CompositionAction::RunSource {
                profile_id: profile.id,
            }),
            OperationKind::Run,
            vapor_studio_core::OperationPhase::Running,
            concat!(
                "Building this project and starting the game. ",
                "Open and sign in to Steam if missing Workshop items are needed…",
            ),
        );
    }
}
