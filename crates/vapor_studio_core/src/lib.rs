//! Product-owned workflow contracts for Vapor Studio.
//!
//! This crate models user profiles and GUI operation state. It deliberately
//! leaves content resolution, installation, and publication to Vapor services.

mod catalog;
mod operation;
mod profile;
mod publishing;
mod shell_content;
mod workspace;

pub use catalog::{
    CatalogConflict, CatalogContent, CatalogLocation, CatalogLocationKind, discover_content_catalog,
};
pub use operation::{
    OperationEvent, OperationId, OperationKind, OperationPhase, OperationRequest, OperationStatus,
    Progress,
};
pub use profile::{
    CURRENT_SCHEMA_VERSION, CompositionProfile, ContentIdentity, InstallationState,
    InstalledArtifactRecord, LaunchSettings, ProfileDocument, ProfileRecovery,
    ResolvedCompositionGraph, ResolvedDependencyIdentity, ResolvedNodeIdentity,
    WorkspaceAssociation, add_profile, decode_profiles, duplicate_profile,
    fingerprint_resolved_graph, remove_profile, rename_profile, resolved_graph_matches,
};
pub use publishing::{
    WorkshopPackagePreview, WorkshopPublication, publish_reviewed_workshop_content,
    stage_workshop_preview,
};
pub use shell_content::{
    CompositionPreview, CompositionPreviewDependency, CompositionPreviewNode,
    CompositionPreviewSource, InstallationCheck, InstalledArtifact, InstalledComposition,
    ProfileRunResult, WorkshopAcquisition, default_packagepack_selector, download_workshop_item,
    download_workshop_items, install_profile_composition, preview_profile_composition,
    repair_profile_composition, run_installed_profile, run_source_profile,
    update_profile_composition, verify_installation,
};
pub use vapor_steam::{
    MAX_SUBSCRIPTION_BATCH_SIZE as MAX_WORKSHOP_SUBSCRIPTION_BATCH, search_workshop,
    subscribe_item as subscribe_workshop_item, subscribe_items as subscribe_workshop_items,
    subscribed_items as workshop_subscriptions, unsubscribe_item as unsubscribe_workshop_item,
    unsubscribe_items as unsubscribe_workshop_items,
};
pub use vapor_steam::{WorkshopItem, WorkshopPage, WorkshopSubscription};

pub use workspace::{
    SourceComposition, SourceCompositionLink, SourceContent, SourceRuntimeOutputs,
    SourceWorkshopMetadata, SourceWorkshopPolicy, UnregisteredProjectCandidate,
    UnregisteredProjectScan, WorkshopPolicyDraft, adopt_workspace_content_project,
    create_basic_content_workspace, create_content_workspace,
    create_content_workspace_with_workshop, discover_source_content, load_source_composition,
    load_source_runtime_outputs, load_source_workshop_metadata, remember_workspace,
    save_source_composition, save_source_runtime_outputs, save_source_workshop_metadata,
    scan_unregistered_content_projects, source_dependency_allowed, source_dependency_choice_group,
    source_dependency_relationship, source_project_kind, source_required_dependency_choice_groups,
    unregister_workspace_content_project, validate_profile_source, validate_workspace,
    validate_workspace_source,
};
