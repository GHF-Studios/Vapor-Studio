use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationId(pub u64);

impl OperationId {
    pub fn fresh() -> Self { Self(NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed)) }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationKind { Discover, Resolve, Acquire, Install, Verify, Update, Repair, CreateWorkspace, CreateContent, CreateWorkspaceFile, OpenWorkspace, DiscoverWorkspace, OpenWorkspaceFile, SaveWorkspaceFile, Fmt, Check, Test, Build, Clean, Validate, Run, RunInstalled, StageWorkshop, PublishWorkshop, StageRootDistribution }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationPhase { Queued, Resolving, Acquiring, Installing, Verifying, Building, Running, Staging, Publishing, Finished }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum OperationStatus { Running, Succeeded { summary: String }, Failed { message: String }, Cancelled }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress { pub completed: u64, pub total: Option<u64>, pub unit: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationRequest {
    pub id: OperationId,
    pub profile_id: Option<String>,
    pub kind: OperationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationEvent {
    pub id: OperationId,
    pub profile_id: Option<String>,
    pub kind: OperationKind,
    pub phase: OperationPhase,
    pub progress: Option<Progress>,
    pub status: OperationStatus,
}
