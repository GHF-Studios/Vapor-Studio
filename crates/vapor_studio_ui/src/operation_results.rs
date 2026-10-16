use super::StudioApp;
use super::operations::OperationResult;

impl StudioApp {
    pub(super) fn apply_operation_result(&mut self, result: OperationResult) {
        match result {
            OperationResult::Workspace(Ok(workspace)) => self.apply_workspace_loaded(workspace),
            OperationResult::Workspace(Err(error)) => self.status = error,
            OperationResult::WorkspaceSelection(Err(error)) => self.status = error,
            OperationResult::UnregisteredProjectScan(Err(error)) => self.status = error,
            OperationResult::RememberedSources(Err(error)) => self.status = error,
            OperationResult::RememberedSources(Ok(known)) => {
                self.apply_remembered_sources(known)
            }
            OperationResult::AdoptedContentProject(Ok((summary, contents))) => {
                self.apply_adopted_content_project(summary, contents)
            }
            OperationResult::UnregisteredContentProject(Ok((summary, contents, identity))) => {
                self.apply_unregistered_content_project(summary, contents, identity)
            }
            OperationResult::UnregisteredProjectScan(Ok(scan)) => {
                self.apply_unregistered_project_scan(scan)
            }
            OperationResult::WorkspaceSelection(Ok((root, contents, catalog, files))) => {
                self.activate_workspace(root, contents, catalog, files)
            }
            OperationResult::Discovery(source_result, catalog_result) => {
                self.apply_catalog_discovery(source_result, catalog_result)
            }
            OperationResult::Message(Err(error))
            | OperationResult::CompositionPreview(Err(error))
            | OperationResult::WorkspaceFiles(Err(error))
            | OperationResult::WorkspaceFileCreated(Err(error))
            | OperationResult::FileContents(Err(error))
            | OperationResult::FileSaved(Err(error))
            | OperationResult::ContentWorkspace(Err(error))
            | OperationResult::SourceCompositionLoaded(Err(error))
            | OperationResult::SourceCompositionSaved(Err(error))
            | OperationResult::SourceWorkshopLoaded(Err(error))
            | OperationResult::SourceWorkshopSaved(Err(error))
            | OperationResult::SourceRuntimeOutputsLoaded(Err(error))
            | OperationResult::SourceRuntimeOutputsSaved(Err(error))
            | OperationResult::UnregisteredContentProject(Err(error))
            | OperationResult::AdoptedContentProject(Err(error)) => self.status = error,
            OperationResult::Verify(Err(error)) => self.apply_verification_error(error),
            OperationResult::Install(Err(error))
            | OperationResult::Update(Err(error))
            | OperationResult::RunProfile(Err(error))
            | OperationResult::Repair(Err(error)) => self.apply_composition_action_error(error),
            OperationResult::Message(Ok(message)) => self.status = message,
            OperationResult::CompositionPreview(Ok(preview)) => {
                self.apply_composition_preview(preview)
            }
            OperationResult::WorkshopStage(Err(error)) => {
                self.apply_workshop_stage_error(error)
            }
            OperationResult::WorkshopAcquisition(Err(error))
            | OperationResult::WorkshopPublish(Err(error))
            | OperationResult::WorkshopSearch(Err(error), _)
            | OperationResult::WorkshopSubscriptions(Err(error)) => {
                self.apply_workshop_error(error)
            }
            OperationResult::WorkshopAcquisition(Ok(acquisition)) => {
                self.apply_workshop_acquisition(acquisition)
            }
            OperationResult::WorkshopPublish(Ok(publication)) => {
                self.apply_workshop_publication(publication)
            }
            OperationResult::WorkshopStage(Ok(preview)) => {
                self.apply_workshop_stage(preview)
            }
            OperationResult::WorkshopSearch(Ok(page), query) => {
                self.apply_workshop_search(page, query)
            }
            OperationResult::WorkshopSubscriptions(Ok(subscriptions)) => {
                self.apply_workshop_subscriptions(subscriptions)
            }
            OperationResult::WorkshopSubscriptionChange(Ok(()), id, subscribed) => {
                self.apply_workshop_subscription_change(id, subscribed)
            }
            OperationResult::WorkshopSubscriptionChange(Err(error), _, _) => {
                self.apply_workshop_error(error)
            }
            OperationResult::WorkspaceFiles(Ok(files)) => {
                self.apply_workspace_files(files)
            }
            OperationResult::WorkspaceFileCreated(Ok(path)) => {
                self.apply_workspace_file_created(path)
            }
            OperationResult::FileContents(Ok((path, contents))) => {
                self.apply_workspace_file_opened(path, contents)
            }
            OperationResult::FileSaved(Ok(path)) => {
                self.apply_workspace_file_saved(path)
            }
            OperationResult::ContentWorkspace(Ok((manifest, contents, files))) => {
                self.apply_content_workspace_created(manifest, contents, files)
            }
            OperationResult::SourceCompositionLoaded(Ok(composition)) => {
                self.apply_source_composition_loaded(composition)
            }
            OperationResult::SourceCompositionSaved(Ok(composition)) => {
                self.apply_source_composition_saved(composition)
            }
            OperationResult::SourceWorkshopLoaded(Ok(metadata)) => {
                self.apply_source_workshop_loaded(metadata)
            }
            OperationResult::SourceWorkshopSaved(Ok(metadata)) => {
                self.apply_source_workshop_saved(metadata)
            }
            OperationResult::SourceRuntimeOutputsLoaded(Ok(outputs)) => {
                self.apply_source_runtime_outputs_loaded(outputs)
            }
            OperationResult::SourceRuntimeOutputsSaved(Ok(outputs)) => {
                self.apply_source_runtime_outputs_saved(outputs)
            }
            OperationResult::Install(Ok(composition)) => {
                self.apply_installed_composition(
                    composition,
                    "Installed the complete resolved composition through Vapor Shell.",
                );
            }
            OperationResult::Update(Ok(composition)) => {
                self.apply_installed_composition(
                    composition,
                    "Updated the selected composition from its current workspace or cache sources.",
                );
            }
            OperationResult::RunProfile(Ok(composition)) => {
                self.apply_installed_composition(
                    composition,
                    "Finished playing the selected source composition.",
                );
            }
            OperationResult::Repair(Ok(composition)) => {
                self.apply_installed_composition(
                    composition,
                    "Repaired the selected composition through Vapor Shell.",
                );
            }
            OperationResult::Verify(Ok(check)) => {
                self.apply_installation_check(check);
            }
        }
    }
}
