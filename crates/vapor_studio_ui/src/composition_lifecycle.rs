use super::StudioApp;
use std::path::PathBuf;
use vapor_client_application::OperationKind;
use vapor_client_protocol::ClientAction;

pub(super) enum RepreviewAfterCatalogRefresh {
    ActiveComposition,
    Source {
        selector: String,
        workspace_root: PathBuf,
    },
}

#[derive(Default)]
pub(super) struct CompositionPreviewState {
    preview: Option<vapor_client_application::CompositionPreview>,
    repreview_after_catalog_refresh: Option<RepreviewAfterCatalogRefresh>,
}

impl CompositionPreviewState {
    pub(super) fn preview(&self) -> Option<&vapor_client_application::CompositionPreview> {
        self.preview.as_ref()
    }

    pub(super) fn set_preview(&mut self, preview: vapor_client_application::CompositionPreview) {
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
        self.repreview_after_catalog_refresh =
            Some(RepreviewAfterCatalogRefresh::ActiveComposition);
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
        preview: vapor_client_application::CompositionPreview,
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
        self.composition_preview.set_preview(preview);
    }

    pub(super) fn adopt_installed_composition_graph(&mut self) {
        let Some(composition) = self.composition.items.get(self.composition.active_index) else {
            return;
        };
        if !matches!(
            &composition.installation,
            vapor_client_application::InstallationState::OutOfDate { verified: true, .. }
        ) {
            return;
        }
        let composition_id = composition.id.clone();
        match self.apply_service_action(ClientAction::Composition(
            vapor_client_protocol::CompositionAction::AdoptVerifiedGraph { composition_id },
        )) {
            Ok(()) => {
                self.status = "Saved the verified installed content to this composition.".into()
            }
            Err(error) => self.status = error,
        }
    }

    pub(super) fn install_selected(&mut self) {
        if !self.require_saved_workspace("install this workspace composition") {
            return;
        }
        let Some(composition_id) = self
            .composition
            .items
            .get(self.composition.active_index)
            .map(|p| p.id.clone())
        else {
            return;
        };
        self.start_service_composition_operation(
            ClientAction::Composition(vapor_client_protocol::CompositionAction::Install {
                composition_id,
            }),
            OperationKind::Install,
            vapor_client_application::OperationPhase::Acquiring,
            "Installing the selected composition and dependencies…",
        );
    }

    pub(super) fn update_selected(&mut self) {
        if !self.require_saved_workspace("update this workspace composition") {
            return;
        }
        let Some(composition_id) = self
            .composition
            .items
            .get(self.composition.active_index)
            .map(|p| p.id.clone())
        else {
            return;
        };
        self.start_service_composition_operation(
            ClientAction::Composition(vapor_client_protocol::CompositionAction::Update {
                composition_id,
            }),
            OperationKind::Update,
            vapor_client_application::OperationPhase::Acquiring,
            "Updating the selected composition…",
        );
    }

    pub(super) fn preview_selected_composition(&mut self) {
        if !self.require_saved_workspace("resolve this workspace composition") {
            return;
        }
        let Some(composition_id) = self
            .composition
            .items
            .get(self.composition.active_index)
            .map(|p| p.id.clone())
        else {
            return;
        };
        self.start_service_composition_operation(
            ClientAction::Composition(vapor_client_protocol::CompositionAction::Preview {
                composition_id,
                selector: None,
            }),
            OperationKind::Resolve,
            vapor_client_application::OperationPhase::Resolving,
            "Checking this composition against available and installed content…",
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
            "Checking the source composition against available and installed content…",
        );
    }

    pub(super) fn preview_composition(
        &mut self,
        selector: String,
        _workspace: Option<PathBuf>,
        status: &str,
    ) {
        let Some(composition_id) = self
            .composition
            .items
            .get(self.composition.active_index)
            .map(|p| p.id.clone())
        else {
            return;
        };
        self.composition_preview.invalidate_preview();
        self.start_service_composition_operation(
            ClientAction::Composition(vapor_client_protocol::CompositionAction::Preview {
                composition_id,
                selector: Some(selector),
            }),
            OperationKind::Resolve,
            vapor_client_application::OperationPhase::Resolving,
            status,
        );
    }

    pub(super) fn verify_selected_installation(&mut self) {
        let Some(composition_id) = self
            .composition
            .items
            .get(self.composition.active_index)
            .map(|composition| composition.id.clone())
        else {
            return;
        };
        self.start_service_composition_operation(
            ClientAction::Composition(vapor_client_protocol::CompositionAction::Verify {
                composition_id,
            }),
            OperationKind::Verify,
            vapor_client_application::OperationPhase::Verifying,
            "Verifying installed composition…",
        );
    }

    pub(super) fn repair_selected_installation(&mut self) {
        if !self.require_saved_workspace("repair this workspace composition") {
            return;
        }
        let Some(composition_id) = self
            .composition
            .items
            .get(self.composition.active_index)
            .map(|composition| composition.id.clone())
        else {
            return;
        };
        self.start_service_composition_operation(
            ClientAction::Composition(vapor_client_protocol::CompositionAction::Repair {
                composition_id,
            }),
            OperationKind::Repair,
            vapor_client_application::OperationPhase::Acquiring,
            "Repairing installed composition…",
        );
    }

    pub(super) fn run_installed_selected(&mut self) {
        let Some(composition) = self
            .composition
            .items
            .get(self.composition.active_index)
            .cloned()
        else {
            return;
        };
        if matches!(
            &composition.installation,
            vapor_client_application::InstallationState::OutOfDate { .. }
        ) {
            self.status = "Review or update this composition before launching.".to_owned();
            return;
        }
        if composition.root_package.is_none() {
            return;
        }
        self.start_service_composition_operation(
            ClientAction::Composition(vapor_client_protocol::CompositionAction::Launch { composition_id: composition.id }),
            OperationKind::RunInstalled,
            vapor_client_application::OperationPhase::Running,
            "Preparing composition. Steam must be open and signed in if Workshop items need downloading…",
        );
    }

    pub(super) fn ensure_default_composition_root(&mut self) -> bool {
        if self.composition.items[self.composition.active_index]
            .root_package
            .is_some()
        {
            return true;
        }
        let composition_id = self.composition.items[self.composition.active_index]
            .id
            .clone();
        match self.apply_service_action(ClientAction::Composition(
            vapor_client_protocol::CompositionAction::ResolveDefaultRoot { composition_id },
        )) {
            Ok(()) => true,
            Err(error) => {
                self.status = error;
                false
            }
        }
    }

    pub(super) fn launch_default_composition(&mut self) {
        if self.ensure_default_composition_root() {
            self.run_installed_selected();
        }
    }

    pub(super) fn run_selected(&mut self) {
        if !self.require_open_workspace("build and play this workspace composition") {
            return;
        }
        let Some(composition) = self
            .composition
            .items
            .get(self.composition.active_index)
            .cloned()
        else {
            return;
        };
        if composition.root_package.is_none() || composition.workspace.is_none() {
            return;
        }
        self.start_service_composition_operation(
            ClientAction::Composition(vapor_client_protocol::CompositionAction::RunSource {
                composition_id: composition.id,
            }),
            OperationKind::Run,
            vapor_client_application::OperationPhase::Running,
            concat!(
                "Building this project and starting the game. ",
                "Open and sign in to Steam if missing Workshop items are needed…",
            ),
        );
    }
}
