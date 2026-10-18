use crate::navigation::Section;
use std::path::PathBuf;
use vapor_core::VaporWorkspace;

pub(super) struct WorkspaceOpenSnapshot {
    pub(super) root: PathBuf,
    pub(super) landing_section: Section,
    pub(super) workspace_files: Result<Vec<PathBuf>, String>,
}
pub(super) enum OperationResult {
    Workspace(Box<Result<VaporWorkspace, String>>),
    Message(Result<String, String>),
}
impl OperationResult {
    pub(super) fn failure(&self) -> Option<String> {
        match self {
            Self::Workspace(result) => result.as_ref().as_ref().err().cloned(),
            Self::Message(Err(error)) => Some(error.clone()),
            _ => None,
        }
    }
}
