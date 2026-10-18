use crate::navigation::Section;
use std::path::PathBuf;
use vapor_core::VaporWorkspace;

pub(super) struct WorkspaceOpenSnapshot {
    pub(super) root: PathBuf,
    pub(super) landing_section: Section,
    pub(super) workspace_files: Result<Vec<PathBuf>, String>,
}
pub(super) enum OperationResult {
    Workspace(Result<VaporWorkspace, String>),
    Message(Result<String, String>),
    RootPublish(Result<vapor_sdk_core::root::types::RootPublishReport, String>),
}
impl OperationResult {
    pub(super) fn failure(&self) -> Option<String> {
        match self {
            Self::Workspace(Err(error))
            | Self::Message(Err(error))
            | Self::RootPublish(Err(error)) => Some(error.clone()),
            Self::RootPublish(Ok(report))
                if report
                    .status
                    .as_ref()
                    .is_some_and(|status| !status.success()) =>
            {
                Some(format!(
                    "Steam root upload failed with {}",
                    report
                        .status
                        .as_ref()
                        .map_or_else(|| "unknown status".to_owned(), ToString::to_string)
                ))
            }
            _ => None,
        }
    }
}
