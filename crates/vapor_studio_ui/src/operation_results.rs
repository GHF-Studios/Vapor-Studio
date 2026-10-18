use super::StudioApp;
use super::operation_protocol::OperationResult;

impl StudioApp {
    pub(super) fn apply_operation_result(&mut self, result: OperationResult) {
        match result {
            OperationResult::Workspace(Ok(workspace)) => self.apply_workspace_loaded(workspace),
            OperationResult::Workspace(Err(error)) => self.status = error,
            OperationResult::Message(Err(error)) => {
                if self
                    .operation
                    .active_operation
                    .as_ref()
                    .is_some_and(|operation| {
                        matches!(
                            &operation.kind,
                            vapor_studio_core::OperationKind::SearchWorkshop
                        )
                    })
                {
                    self.apply_workshop_error(error);
                } else {
                    self.status = error;
                }
            }
            OperationResult::Message(Ok(message)) => self.status = message,
            OperationResult::RootPublish(Ok(report)) => {
                self.root_publishing.preview = Some(report.clone());
                if report.planned {
                    self.root_publishing.status = format!(
                        "Steam root review ready · App {} · depot {} · {} files · account {}.",
                        report.package.app_id,
                        report.package.depot_id,
                        report.package.copied_files,
                        report.account,
                    );
                } else if report
                    .status
                    .as_ref()
                    .is_some_and(|status| status.success())
                {
                    self.root_publishing.confirmation.clear();
                    self.root_publishing.status = format!(
                        "Steam upload completed · App {} · depot {} · account {} · branch {}.",
                        report.package.app_id,
                        report.package.depot_id,
                        report.account,
                        report.set_live.as_deref().unwrap_or("not changed"),
                    );
                } else if let Some(status) = report.status.as_ref() {
                    self.root_publishing.status =
                        format!("Steam root upload failed with {status}.");
                }
                self.status = self.root_publishing.status.clone();
            }
            OperationResult::RootPublish(Err(error)) => {
                self.root_publishing.status = error.clone();
                self.status = error;
            }
        }
    }
}
