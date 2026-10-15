//! Product-owned workflow contracts for Vapor Studio.
//!
//! This crate models user profiles and GUI operation state. It deliberately
//! leaves content resolution, installation, and publication to Vapor services.

mod operation;
mod profile;
mod catalog;
mod publishing;
mod shell_content;
mod workspace;

pub use operation::{OperationEvent, OperationId, OperationKind, OperationPhase, OperationRequest, OperationStatus, Progress};
pub use publishing::{WorkshopPackagePreview, WorkshopPublication, publish_reviewed_workshop_content, stage_workshop_preview};
pub use catalog::{CatalogContent, CatalogLocation, CatalogLocationKind, discover_content_catalog};
pub use vapor_steam::{WorkshopItem, WorkshopPage, WorkshopSubscription};
pub use vapor_steam::{search_workshop, subscribe_item as subscribe_workshop_item, subscribed_items as workshop_subscriptions, unsubscribe_item as unsubscribe_workshop_item};
pub use shell_content::{
    CompositionPreview, CompositionPreviewDependency, CompositionPreviewNode,
    CompositionPreviewSource, InstalledArtifact, InstalledComposition, InstallationCheck, WorkshopAcquisition,
    default_packagepack_selector, download_workshop_item, install_profile_composition,
    preview_profile_composition,
    repair_profile_composition, run_installed_profile, run_source_profile, update_profile_composition,
    verify_installation,
};
pub use profile::{
    CompositionProfile, ContentIdentity, InstallationState, InstalledArtifactRecord, LaunchSettings, ResolvedCompositionGraph, ResolvedDependencyIdentity, ResolvedNodeIdentity,
    ProfileDocument, ProfileRecovery, WorkspaceAssociation, CURRENT_SCHEMA_VERSION,
    add_profile, decode_profiles, duplicate_profile,
};

pub use workspace::{SourceContent, create_basic_content_workspace, create_content_workspace, discover_source_content, source_dependency_allowed, validate_profile_source, validate_workspace_source};
