use super::StudioApp;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    time::{SystemTime, UNIX_EPOCH},
};
use vapor_client_application::{
    OperationEvent, OperationId, OperationKind, OperationPhase, OperationStatus,
};
use vapor_client_protocol::{
    ClientAction, OperationEvent as ServiceOperationEvent,
    OperationOutputEvent as ServiceOperationOutputEvent,
    OperationPhase as ServiceOperationPhase,
    OperationStatus as ServiceOperationStatus, RequestId,
};
use vapor_console_core::{ConsoleRecord};
use vapor_gui_core::console::ConsolePanelState;

#[derive(Default)]
pub(super) struct OperationState {
    pub(super) running: bool,
    pub(super) service_owned: bool,
    pub(super) service_request_id: Option<RequestId>,
    pub(super) active_operation: Option<OperationEvent>,
    pub(super) active_operations: VecDeque<OperationEvent>,
    active_service_action_ids: HashMap<String, String>,
    pub(super) operation_history: VecDeque<HistoricalOperation>,
    pub(super) console: ConsolePanelState,
    queued_service_operations: VecDeque<QueuedServiceOperation>,
    pub(super) pending_service_requests: HashMap<RequestId, QueuedServiceOperation>,
}

const MAX_QUEUED_SERVICE_OPERATIONS: usize = 64;

pub(super) struct QueuedServiceOperation {
    pub(super) action: ClientAction,
    pub(super) composition_id: Option<String>,
    pub(super) kind: OperationKind,
    pub(super) phase: OperationPhase,
    pub(super) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct HistoricalOperation {
    pub(super) completed_at_unix: u64,
    pub(super) event: OperationEvent,
}

impl OperationState {
    pub(super) fn clear_queued_service_operations(&mut self) {
        self.queued_service_operations.clear();
        self.pending_service_requests.clear();
        self.active_operations.clear();
        self.active_service_action_ids.clear();
        self.active_operation = None;
        self.sync_activity_state();
    }

    pub(super) fn queue_conflicting_service_operation(
        &mut self,
        operation: QueuedServiceOperation,
    ) -> Result<usize, ()> {
        self.enqueue_service_operation(operation)
    }

    fn enqueue_service_operation(
        &mut self,
        operation: QueuedServiceOperation,
    ) -> Result<usize, ()> {
        if self.queued_service_operations.len() >= MAX_QUEUED_SERVICE_OPERATIONS {
            return Err(());
        }
        self.queued_service_operations.push_back(operation);
        Ok(self.queued_service_operations.len())
    }

    pub(super) fn restore_history(&mut self, encoded: Option<&str>) {
        let Some(history) = encoded
            .and_then(|value| serde_json::from_str::<VecDeque<HistoricalOperation>>(value).ok())
        else {
            return;
        };
        self.operation_history = history.into_iter().take(20).collect();
    }

    pub(super) fn apply_service_operation_output(&mut self, event: &ServiceOperationOutputEvent) {
        if !self
            .active_operations
            .iter()
            .any(|operation| operation.id.0.to_string() == event.id)
        {
            return;
        }
        for line in event.text.lines().filter(|line| !line.trim().is_empty()) {
            let source = if event.source.is_empty() { "command" } else { &event.source };
            let mut record = ConsoleRecord::new(event.timestamp_unix_ms, event.level, source, line)
                .target(event.action.clone())
                .field("operation", event.id.clone())
                .field("stream", format!("{:?}", event.stream).to_ascii_lowercase());
            if let Some(process) = &event.process {
                record = record.field("process", process.clone());
            }
            self.console.history.push(record);
        }
    }

    fn apply_service_operation_snapshots(
        &mut self,
        operations: &[vapor_client_protocol::OperationSummary],
    ) {
        let incoming_ids = operations
            .iter()
            .map(|operation| operation.id.clone())
            .collect::<std::collections::HashSet<_>>();
        self.active_operations.retain(|active| {
            active.phase == OperationPhase::Finalizing
                || incoming_ids.contains(&active.id.0.to_string())
        });
        let finalizing_ids = self
            .active_operations
            .iter()
            .filter(|active| active.phase == OperationPhase::Finalizing)
            .map(|active| active.id.0.to_string())
            .collect::<std::collections::HashSet<_>>();
        self.active_service_action_ids
            .retain(|operation_id, _| finalizing_ids.contains(operation_id));
        for operation in operations {
            let id = operation
                .id
                .parse::<u64>()
                .map(OperationId)
                .unwrap_or_else(|_| OperationId::fresh());
            let status = match &operation.status {
                ServiceOperationStatus::Running => OperationStatus::Running,
                ServiceOperationStatus::Cancelling => OperationStatus::Cancelling,
                ServiceOperationStatus::Succeeded => OperationStatus::Succeeded {
                    summary: "Completed".to_owned(),
                },
                ServiceOperationStatus::Failed { message } => OperationStatus::Failed {
                    message: message.clone(),
                },
                ServiceOperationStatus::Cancelled => OperationStatus::Cancelled,
            };
            let event = OperationEvent {
                id,
                composition_id: operation.composition_id.clone(),
                kind: operation_kind_for_action(&operation.action),
                phase: service_phase(operation.phase),
                progress: operation
                    .progress
                    .map(|progress| vapor_client_application::Progress {
                        completed: (progress.clamp(0.0, 1.0) * 100.0).round() as u64,
                        total: Some(100),
                        unit: "percent".to_owned(),
                        message: None,
                    }),
                status,
            };
            self.active_service_action_ids
                .insert(operation.id.clone(), operation.action.clone());
            self.upsert_active_operation(event);
        }
        self.sync_activity_state();
    }

    fn upsert_active_operation(&mut self, operation: OperationEvent) {
        if let Some(existing) = self
            .active_operations
            .iter_mut()
            .find(|existing| existing.id == operation.id)
        {
            *existing = operation;
        } else {
            self.active_operations.push_back(operation);
        }
        self.active_operation = self.active_operations.front().cloned();
    }

    fn remove_active_operation(&mut self, operation_id: &str) -> Option<OperationEvent> {
        let index = self
            .active_operations
            .iter()
            .position(|operation| operation.id.0.to_string() == operation_id)?;
        let operation = self.active_operations.remove(index);
        self.active_service_action_ids.remove(operation_id);
        self.active_operation = self.active_operations.front().cloned();
        operation
    }

    pub(super) fn sync_activity_state(&mut self) {
        self.active_operation = self.active_operations.front().cloned();
        self.service_request_id = self.pending_service_requests.keys().next().cloned();
        self.service_owned =
            !self.active_operations.is_empty() || !self.pending_service_requests.is_empty();
        self.running = self.service_owned || !self.queued_service_operations.is_empty();
    }

    pub(super) fn reconcile_service_request(
        &mut self,
        request_id: &RequestId,
    ) -> Option<QueuedServiceOperation> {
        let operation = self.pending_service_requests.remove(request_id);
        self.sync_activity_state();
        operation
    }

    pub(super) fn service_action_is_pending_or_active(&self, action: &ClientAction) -> bool {
        let action_id = action.id();
        self.active_service_action_ids
            .values()
            .any(|active| active == action_id)
            || self
                .pending_service_requests
                .values()
                .any(|pending| pending.action.id() == action_id)
            || self
                .queued_service_operations
                .iter()
                .any(|queued| queued.action.id() == action_id)
    }

    pub(super) fn service_action_id_is_pending_or_active(&self, action_id: &str) -> bool {
        self.active_service_action_ids
            .values()
            .any(|active| active == action_id)
            || self
                .pending_service_requests
                .values()
                .any(|pending| pending.action.id() == action_id)
            || self
                .queued_service_operations
                .iter()
                .any(|queued| queued.action.id() == action_id)
    }
}

pub(super) fn elapsed_label(completed_at_unix: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let elapsed = now.saturating_sub(completed_at_unix);
    if elapsed < 60 {
        "just now".to_owned()
    } else if elapsed < 3_600 {
        format!("{}m ago", elapsed / 60)
    } else if elapsed < 86_400 {
        format!("{}h ago", elapsed / 3_600)
    } else {
        format!("{}d ago", elapsed / 86_400)
    }
}

impl StudioApp {
    pub(super) fn start_service_composition_operation(
        &mut self,
        action: ClientAction,
        kind: OperationKind,
        phase: OperationPhase,
        status: &str,
    ) {
        let composition_id = self
            .composition
            .items
            .get(self.composition.active_index)
            .map(|composition| composition.id.clone());
        let Some(composition_id) = composition_id else {
            self.status = "Create or select a composition first.".to_owned();
            return;
        };
        self.start_service_operation(action, Some(composition_id), kind, phase, status);
    }

    pub(super) fn start_service_operation(
        &mut self,
        action: ClientAction,
        composition_id: Option<String>,
        kind: OperationKind,
        phase: OperationPhase,
        status: &str,
    ) {
        let Some(service) = self.service.as_mut() else {
            self.status = self
                .service_error
                .clone()
                .unwrap_or_else(|| "Studio service is not connected.".to_owned());
            return;
        };
        let request_action = action.clone();
        let request_id = match service.send(action, None) {
            Ok(request_id) => request_id,
            Err(error) => {
                self.status = format!("Could not start the Studio operation: {error}");
                return;
            }
        };
        self.operation.pending_service_requests.insert(
            request_id,
            QueuedServiceOperation {
                action: request_action,
                composition_id,
                kind,
                phase,
                status: status.to_owned(),
            },
        );
        self.operation.sync_activity_state();
        self.status = status.to_owned();
    }

    pub(super) fn apply_service_operation_event(&mut self, event: &ServiceOperationEvent) {
        if self
            .operation
            .operation_history
            .iter()
            .any(|record| record.event.id.0.to_string() == event.id)
        {
            return;
        }
        let kind = operation_kind_for_action(&event.action);
        let id = event
            .id
            .parse::<u64>()
            .map(OperationId)
            .unwrap_or_else(|_| OperationId::fresh());
        let mut operation = self
            .operation
            .active_operations
            .iter()
            .find(|operation| operation.id == id)
            .cloned()
            .unwrap_or(OperationEvent {
                id,
                composition_id: event.composition_id.clone(),
                kind: kind.clone(),
                phase: OperationPhase::Queued,
                progress: None,
                status: OperationStatus::Running,
            });
        operation.composition_id = event.composition_id.clone();
        operation.kind = kind;
        operation.phase = service_phase(event.phase);
        operation.progress = event
            .progress
            .map(|progress| vapor_client_application::Progress {
                completed: (progress.clamp(0.0, 1.0) * 100.0).round() as u64,
                total: Some(100),
                unit: "percent".to_owned(),
                message: event.message.clone(),
            });
        operation.status = match &event.status {
            ServiceOperationStatus::Running => OperationStatus::Running,
            ServiceOperationStatus::Cancelling => OperationStatus::Cancelling,
            ServiceOperationStatus::Succeeded => OperationStatus::Succeeded {
                summary: event
                    .message
                    .clone()
                    .unwrap_or_else(|| "Completed".to_owned()),
            },
            ServiceOperationStatus::Failed { message } => OperationStatus::Failed {
                message: message.clone(),
            },
            ServiceOperationStatus::Cancelled => OperationStatus::Cancelled,
        };
        let terminal = matches!(
            &operation.status,
            OperationStatus::Succeeded { .. }
                | OperationStatus::Failed { .. }
                | OperationStatus::Cancelled
        );
        if terminal {
            operation.phase = OperationPhase::Finalizing;
        }
        let completed_event = operation.clone();
        let status_message = event.message.clone();
        let awaiting_workspace_result = terminal
            && event.action.starts_with("workspace.")
            && matches!(&event.status, ServiceOperationStatus::Succeeded);
        let awaiting_authoring_result = terminal
            && event.action.starts_with("authoring.")
            && matches!(&event.status, ServiceOperationStatus::Succeeded);
        if !terminal || awaiting_workspace_result || awaiting_authoring_result {
            self.operation
                .active_service_action_ids
                .insert(event.id.clone(), event.action.clone());
        }
        if let Some(message) = status_message {
            self.status = message.clone();
            if event.action.starts_with("publishing.") {
                self.root_publishing.status = message;
            }
        }
        if terminal {
            let completed_at_unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            self.operation
                .operation_history
                .push_front(HistoricalOperation {
                    completed_at_unix,
                    event: completed_event.clone(),
                });
            self.operation.operation_history.truncate(20);
            let awaiting_result = awaiting_workspace_result || awaiting_authoring_result;
            if awaiting_result {
                self.operation.upsert_active_operation(completed_event);
            } else {
                self.operation.remove_active_operation(&event.id);
            }
            match event.action.as_str() {
                "workshop.search"
                    if matches!(&event.status, ServiceOperationStatus::Failed { .. }) =>
                {
                    self.apply_workshop_error(
                        event
                            .message
                            .clone()
                            .unwrap_or_else(|| "Workshop search failed.".to_owned()),
                    );
                }
                "workshop.acquire"
                    if matches!(&event.status, ServiceOperationStatus::Failed { .. }) =>
                {
                    self.apply_workshop_acquisition_error(
                        event
                            .message
                            .clone()
                            .unwrap_or_else(|| "Workshop acquisition failed.".to_owned()),
                    );
                }
                "workshop.stage"
                    if matches!(&event.status, ServiceOperationStatus::Failed { .. }) =>
                {
                    self.apply_workshop_stage_error(
                        event
                            .message
                            .clone()
                            .unwrap_or_else(|| "Workshop staging failed.".to_owned()),
                    );
                }
                "workshop.publish"
                    if matches!(&event.status, ServiceOperationStatus::Failed { .. }) =>
                {
                    self.apply_workshop_error(
                        event
                            .message
                            .clone()
                            .unwrap_or_else(|| "Workshop publishing failed.".to_owned()),
                    );
                }
                "workshop.publish"
                    if matches!(&event.status, ServiceOperationStatus::Succeeded) =>
                {
                    self.workshop.confirmation.clear();
                }
                "workshop.load-subscriptions" | "workshop.subscribe" | "workshop.unsubscribe"
                    if matches!(&event.status, ServiceOperationStatus::Failed { .. }) =>
                {
                    self.apply_workshop_subscriptions_error(event.message.clone().unwrap_or_else(
                        || "Workshop subscriptions could not be loaded.".to_owned(),
                    ));
                }
                "workshop.subscribe" | "workshop.unsubscribe"
                    if matches!(&event.status, ServiceOperationStatus::Succeeded) =>
                {
                    self.workshop.selected_items.clear();
                }
                "workspace.save-file"
                    if matches!(&event.status, ServiceOperationStatus::Failed { .. }) =>
                {
                    self.fail_service_workspace_save(
                        event
                            .message
                            .clone()
                            .unwrap_or_else(|| "Workspace file save failed.".to_owned()),
                    );
                }
                _ => {}
            }
            if !awaiting_result {
                self.operation.sync_activity_state();
                self.checkpoint_service_local_state();
                self.start_next_service_operation();
            }
        } else {
            self.operation.upsert_active_operation(operation);
            self.operation.sync_activity_state();
        }
    }

    pub(super) fn finish_service_result_operation(&mut self, operation_id: &str) {
        if self
            .operation
            .remove_active_operation(operation_id)
            .is_none()
        {
            return;
        }
        self.operation.sync_activity_state();
        self.checkpoint_service_local_state();
        self.start_next_service_operation();
    }

    fn checkpoint_service_local_state(&mut self) {
        let editor_drafts = self
            .workspace
            .editor
            .save_editor_drafts(&self.workspace.root);
        let operation_history = serde_json::to_string(&self.operation.operation_history).ok();
        self.submit_service_action(ClientAction::Session(
            vapor_client_protocol::SessionAction::CheckpointLocalState {
                editor_drafts,
                operation_history,
            },
        ));
    }

    pub(super) fn apply_service_operation_snapshot(
        &mut self,
        operations: &[vapor_client_protocol::OperationSummary],
    ) {
        self.operation.apply_service_operation_snapshots(operations);
    }

    pub(super) fn start_next_service_operation(&mut self) {
        let Some(next) = self.operation.queued_service_operations.pop_front() else {
            return;
        };
        self.start_service_operation(
            next.action,
            next.composition_id,
            next.kind,
            next.phase,
            &next.status,
        );
        self.operation.sync_activity_state();
    }
}

fn operation_kind_for_action(action: &str) -> OperationKind {
    match action {
        "workshop.search" => OperationKind::SearchWorkshop,
        "workshop.acquire" => OperationKind::Acquire,
        "workshop.stage" => OperationKind::StageWorkshop,
        "workshop.publish" => OperationKind::PublishWorkshop,
        "publishing.stage-root" => OperationKind::StageRootDistribution,
        "publishing.publish-root" => OperationKind::PublishRootDistribution,
        "workshop.load-subscriptions" => OperationKind::LoadWorkshopSubscriptions,
        "workshop.subscribe" | "workshop.unsubscribe" => OperationKind::ChangeWorkshopSubscription,
        "library.set-auto-install" => OperationKind::ChangeWorkshopSubscription,
        "composition.preview" => OperationKind::Resolve,
        "composition.install" => OperationKind::Install,
        "composition.verify" => OperationKind::Verify,
        "composition.repair" => OperationKind::Repair,
        "composition.update" => OperationKind::Update,
        "composition.launch" => OperationKind::RunInstalled,
        "composition.run-source" => OperationKind::Run,
        "workspace.validate" => OperationKind::Validate,
        "workspace.fmt" => OperationKind::Fmt,
        "workspace.check" => OperationKind::Check,
        "workspace.test" => OperationKind::Test,
        "workspace.build" => OperationKind::Build,
        "workspace.clean" => OperationKind::Clean,
        "workspace.list-files" => OperationKind::DiscoverWorkspace,
        "workspace.remember" => OperationKind::RememberWorkspace,
        "workspace.adopt-project" => OperationKind::AdoptContentProject,
        "workspace.unregister-project" => OperationKind::UnregisterContentProject,
        "authoring.create-workspace" => OperationKind::CreateWorkspace,
        "authoring.create-content" => OperationKind::CreateContent,
        "authoring.fork-content" => OperationKind::ForkContent,
        "authoring.load-composition" => OperationKind::LoadComposition,
        "authoring.save-composition" => OperationKind::SaveComposition,
        "authoring.load-workshop-metadata" => OperationKind::LoadWorkshopMetadata,
        "authoring.save-workshop-metadata" => OperationKind::SaveWorkshopMetadata,
        "authoring.load-runtime-outputs" => OperationKind::LoadRuntimeOutputs,
        "authoring.validate-content" => OperationKind::Validate,
        "authoring.package-content" => OperationKind::PackageContent,
        "authoring.list-registered-sources" => OperationKind::Discover,
        "authoring.save-runtime-outputs" => OperationKind::SaveRuntimeOutputs,
        _ => OperationKind::Resolve,
    }
}

fn service_phase(phase: ServiceOperationPhase) -> OperationPhase {
    match phase {
        ServiceOperationPhase::Queued => OperationPhase::Queued,
        ServiceOperationPhase::Resolving => OperationPhase::Resolving,
        ServiceOperationPhase::Acquiring => OperationPhase::Acquiring,
        ServiceOperationPhase::Installing => OperationPhase::Installing,
        ServiceOperationPhase::Verifying => OperationPhase::Verifying,
        ServiceOperationPhase::Building => OperationPhase::Building,
        ServiceOperationPhase::Staging => OperationPhase::Staging,
        ServiceOperationPhase::Publishing => OperationPhase::Publishing,
        ServiceOperationPhase::Running => OperationPhase::Running,
        ServiceOperationPhase::Finalizing | ServiceOperationPhase::Complete => {
            OperationPhase::Finalizing
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::{MAX_QUEUED_SERVICE_OPERATIONS, OperationState, QueuedServiceOperation};
    use vapor_client_application::{OperationEvent, OperationId, OperationKind, OperationPhase};
    use vapor_client_protocol::{
        ClientAction, OperationOutputEvent, OperationOutputStream, OperationPhase as ServicePhase,
        OperationStatus as ServiceStatus, RequestId,
    };

    #[test]
    fn output_is_retained_as_a_structured_shared_console_record() {
        let mut state = OperationState {
            active_operation: Some(OperationEvent {
                id: OperationId(7),
                composition_id: None,
                kind: OperationKind::Check,
                phase: OperationPhase::Running,
                progress: None,
                status: super::OperationStatus::Running,
            }),
            active_operations: VecDeque::from([OperationEvent {
                id: OperationId(7),
                composition_id: None,
                kind: OperationKind::Check,
                phase: OperationPhase::Running,
                progress: None,
                status: super::OperationStatus::Running,
            }]),
            ..OperationState::default()
        };

        state.apply_service_operation_output(&OperationOutputEvent {
            id: "7".to_owned(),
            stream: OperationOutputStream::Stderr,
            timestamp_unix_ms: 123,
            level: vapor_client_protocol::ConsoleLevel::Error,
            source: "workspace".to_owned(),
            action: "workspace.test".to_owned(),
            process: Some("cargo".to_owned()),
            text: "compiler stopped".to_owned(),
        });
        let record = state.console.history.records().next().unwrap();
        assert_eq!(record.timestamp_unix_ms, 123);
        assert_eq!(record.level, vapor_client_protocol::ConsoleLevel::Error);
        assert_eq!(record.source.0, "workspace");
        assert_eq!(record.target.as_deref(), Some("workspace.test"));
        assert!(record.fields.iter().any(|field| field.key == "process" && field.value == "cargo"));
    }

    #[test]
    fn service_snapshot_reconciles_each_active_operation_independently() {
        let mut state = OperationState {
            running: true,
            service_owned: true,
            ..OperationState::default()
        };

        state.apply_service_operation_snapshots(&[
            vapor_client_protocol::OperationSummary {
                id: "7".to_owned(),
                composition_id: Some("default".to_owned()),
                action: "composition.preview".to_owned(),
                phase: ServicePhase::Resolving,
                status: ServiceStatus::Running,
                progress: Some(0.5),
            },
            vapor_client_protocol::OperationSummary {
                id: "8".to_owned(),
                composition_id: Some("other".to_owned()),
                action: "workshop.search".to_owned(),
                phase: ServicePhase::Resolving,
                status: ServiceStatus::Running,
                progress: None,
            },
        ]);
        assert_eq!(state.active_operations.len(), 2);
        assert!(state.service_action_id_is_pending_or_active("composition.preview"));
        assert!(state.service_action_id_is_pending_or_active("workshop.search"));
        assert_eq!(
            state
                .active_operation
                .as_ref()
                .map(|operation| operation.id),
            Some(OperationId(7))
        );

        state.apply_service_operation_snapshots(&[vapor_client_protocol::OperationSummary {
            id: "8".to_owned(),
            composition_id: Some("other".to_owned()),
            action: "workshop.search".to_owned(),
            phase: ServicePhase::Resolving,
            status: ServiceStatus::Running,
            progress: None,
        }]);

        assert!(state.running);
        assert_eq!(state.active_operations.len(), 1);
        assert_eq!(
            state
                .active_operation
                .as_ref()
                .map(|operation| operation.id),
            Some(OperationId(8))
        );
    }

    #[test]
    fn queued_service_operations_are_bounded_and_fifo() {
        let mut state = OperationState::default();
        for index in 0..MAX_QUEUED_SERVICE_OPERATIONS {
            let queued = state.enqueue_service_operation(QueuedServiceOperation {
                action: vapor_client_protocol::ClientAction::Catalog(
                    vapor_client_protocol::CatalogAction::Refresh,
                ),
                composition_id: None,
                kind: OperationKind::SearchWorkshop,
                phase: OperationPhase::Queued,
                status: format!("operation {index}"),
            });
            assert!(matches!(queued, Ok(count) if count == index + 1));
        }
        assert!(
            state
                .enqueue_service_operation(QueuedServiceOperation {
                    action: vapor_client_protocol::ClientAction::Catalog(
                        vapor_client_protocol::CatalogAction::Refresh,
                    ),
                    composition_id: None,
                    kind: OperationKind::SearchWorkshop,
                    phase: OperationPhase::Queued,
                    status: "overflow".to_owned(),
                })
                .is_err()
        );
        assert_eq!(
            state
                .queued_service_operations
                .pop_front()
                .map(|operation| operation.status),
            Some("operation 0".to_owned())
        );
    }

    #[test]
    fn pending_requests_keep_independent_activity_until_each_response_arrives() {
        let mut state = OperationState::default();
        let refresh = ClientAction::Catalog(vapor_client_protocol::CatalogAction::Refresh);
        let search = ClientAction::Workshop(vapor_client_protocol::WorkshopAction::Search {
            query: "ship".to_owned(),
            page: 1,
            content_kind: None,
            required_tags: Vec::new(),
        });
        let refresh_request = RequestId("refresh-request".to_owned());
        let search_request = RequestId("search-request".to_owned());
        state.pending_service_requests.insert(
            refresh_request.clone(),
            QueuedServiceOperation {
                action: refresh.clone(),
                composition_id: None,
                kind: OperationKind::Discover,
                phase: OperationPhase::Queued,
                status: "Refreshing Library".to_owned(),
            },
        );
        state.pending_service_requests.insert(
            search_request.clone(),
            QueuedServiceOperation {
                action: search.clone(),
                composition_id: None,
                kind: OperationKind::SearchWorkshop,
                phase: OperationPhase::Queued,
                status: "Searching Workshop".to_owned(),
            },
        );
        state.sync_activity_state();

        assert!(state.running);
        assert!(state.service_action_is_pending_or_active(&refresh));
        assert!(state.service_action_is_pending_or_active(&search));
        assert!(state.service_action_id_is_pending_or_active("catalog.refresh"));
        assert!(state.service_action_id_is_pending_or_active("workshop.search"));
        assert_eq!(state.pending_service_requests.len(), 2);

        assert!(state.reconcile_service_request(&refresh_request).is_some());
        assert!(state.running);
        assert!(!state.service_action_is_pending_or_active(&refresh));
        assert!(state.service_action_is_pending_or_active(&search));
        assert!(!state.service_action_id_is_pending_or_active("catalog.refresh"));
        assert!(state.service_action_id_is_pending_or_active("workshop.search"));
        assert_eq!(state.service_request_id, Some(search_request));
    }
}
