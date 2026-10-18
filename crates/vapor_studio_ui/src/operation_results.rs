use super::StudioApp;
use super::operation_protocol::OperationResult;

impl StudioApp {
    pub(super) fn apply_operation_result(&mut self, result: OperationResult) {
        match result {
            OperationResult::Workspace(workspace) => match *workspace {
                Ok(workspace) => self.apply_workspace_loaded(workspace),
                Err(error) => self.status = error,
            },
            OperationResult::Message(Err(error)) => {
                if self
                    .operation
                    .active_operation
                    .as_ref()
                    .is_some_and(|operation| {
                        matches!(
                            &operation.kind,
                            vapor_client_application::OperationKind::SearchWorkshop
                        )
                    })
                {
                    self.apply_workshop_error(error);
                } else {
                    self.status = error;
                }
            }
            OperationResult::Message(Ok(message)) => self.status = message,
        }
    }
}
