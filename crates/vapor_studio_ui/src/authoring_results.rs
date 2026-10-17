use super::StudioApp;
use vapor_studio_core::ContentIdentity;

impl StudioApp {
    pub(super) fn apply_source_composition_loaded(
        &mut self,
        composition: vapor_studio_core::SourceComposition,
    ) {
        self.authoring.composition_link_candidate.clear();
        self.authoring.composition_link_search.clear();
        self.authoring.packagepack_engine_search.clear();
        self.authoring.packagepack_game_search.clear();
        self.authoring.composition_edit_selector = composition
            .version
            .as_deref()
            .map(|version| format!("{}@{version}", composition.id))
            .unwrap_or_else(|| composition.id.clone());
        self.authoring.composition_original_links = Some(composition.links.clone());
        self.authoring.composition_edit = Some(composition);
        self.status = format!(
            "Loaded dependencies for this {}. Review them before saving.",
            self.authoring
                .composition_edit
                .as_ref()
                .map(|graph| graph.kind.as_str())
                .unwrap_or("source content"),
        );
    }

    pub(super) fn apply_source_composition_saved(
        &mut self,
        composition: vapor_studio_core::SourceComposition,
    ) {
        self.invalidate_workshop_review();
        self.authoring.composition_link_candidate.clear();
        self.authoring.composition_link_search.clear();
        self.authoring.packagepack_engine_search.clear();
        self.authoring.packagepack_game_search.clear();
        let identity = ContentIdentity {
            id: composition.id.clone(),
            version: composition.version.clone(),
        };
        self.authoring.composition_original_links = Some(composition.links.clone());
        self.authoring.composition_edit = Some(composition);
        self.invalidate_profiles_for_source_content(&identity);
        self.status =
            "Saved the composition. Studio is refreshing its content catalog and plan.".to_owned();
    }

    pub(super) fn apply_source_workshop_loaded(
        &mut self,
        metadata: vapor_studio_core::SourceWorkshopMetadata,
    ) {
        self.set_workshop_metadata_fields(&metadata.policy);
        self.authoring.workshop_metadata_edit = Some(metadata);
        self.authoring.runtime_outputs_edit = None;
        self.authoring.manifest_metadata_dirty = false;
        self.status =
            "Loaded the source Workshop metadata. Edit the fields, then save through Vapor Shell."
                .to_owned();
    }

    pub(super) fn apply_source_workshop_saved(
        &mut self,
        metadata: vapor_studio_core::SourceWorkshopMetadata,
    ) {
        self.invalidate_workshop_review();
        self.set_workshop_metadata_fields(&metadata.policy);
        self.authoring.workshop_metadata_edit = Some(metadata.clone());
        self.authoring.manifest_metadata_dirty = false;
        self.invalidate_profiles_for_source_content(&ContentIdentity {
            id: metadata.id.clone(),
            version: metadata.version.clone(),
        });
        self.sync_saved_workspace_manifest(&metadata.manifest, &metadata.source);
        self.status = "Saved Workshop metadata through Vapor Shell. Studio is refreshing the workspace catalog.".to_owned();
    }

    pub(super) fn apply_source_runtime_outputs_loaded(
        &mut self,
        outputs: vapor_studio_core::SourceRuntimeOutputs,
    ) {
        self.set_runtime_output_fields(&outputs);
        self.authoring.runtime_outputs_edit = Some(outputs);
        self.authoring.workshop_metadata_edit = None;
        self.authoring.manifest_metadata_dirty = false;
        self.status = "Loaded runtime output declarations. Edit the executable and library lists, then save through Vapor Shell.".to_owned();
    }

    pub(super) fn apply_source_runtime_outputs_saved(
        &mut self,
        outputs: vapor_studio_core::SourceRuntimeOutputs,
    ) {
        self.invalidate_workshop_review();
        self.set_runtime_output_fields(&outputs);
        self.authoring.runtime_outputs_edit = Some(outputs.clone());
        self.authoring.manifest_metadata_dirty = false;
        self.invalidate_profiles_for_source_content(&ContentIdentity {
            id: outputs.id.clone(),
            version: outputs.version.clone(),
        });
        self.sync_saved_workspace_manifest(&outputs.manifest, &outputs.source);
        self.status = "Saved runtime outputs through Vapor Shell. Studio is refreshing the catalog and active profile.".to_owned();
    }
}
