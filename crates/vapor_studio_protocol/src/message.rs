use crate::{StudioAction, StudioEvent, StudioRevision, StudioSnapshot};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudioRequest {
    pub protocol_version: u16,
    pub request_id: RequestId,
    pub expected_revision: Option<StudioRevision>,
    pub action: StudioAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudioResponse {
    pub protocol_version: u16,
    pub request_id: RequestId,
    pub revision: StudioRevision,
    pub kind: StudioResponseKind,
    pub snapshot: StudioSnapshot,
    pub error: Option<StudioError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StudioResponseKind {
    Applied,
    OperationAccepted,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudioError {
    pub code: StudioErrorCode,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StudioErrorCode {
    UnsupportedProtocol,
    InvalidRequest,
    StaleRevision,
    ActionUnavailable,
    OperationInProgress,
    Conflict,
    ProviderFailure,
    PersistenceFailure,
    InternalFailure,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "kebab-case")]
pub enum ProtocolFrame {
    Request(StudioRequest),
    Response(Box<StudioResponse>),
    Event(StudioEvent),
}
