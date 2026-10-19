use crate::navigation::Section;
use std::path::PathBuf;

pub(super) struct WorkspaceOpenSnapshot {
    pub(super) root: PathBuf,
    pub(super) landing_section: Section,
    pub(super) workspace_files: Result<Vec<PathBuf>, String>,
}
