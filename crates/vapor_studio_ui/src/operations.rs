use super::StudioApp;
use super::operation_protocol::OperationResult;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, TryRecvError},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use vapor_client_application::{
    OperationEvent, OperationId, OperationKind, OperationPhase, OperationRequest, OperationStatus,
};
use vapor_client_protocol::{
    ClientAction, OperationEvent as ServiceOperationEvent, OperationPhase as ServiceOperationPhase,
    OperationStatus as ServiceOperationStatus, RequestId,
};
use vapor_core::{
    OperationEvent as CoreOperationEvent, OperationRequest as CoreOperationRequest,
    OperationService,
};

pub(super) struct OperationState {
    pub(super) running: bool,
    pub(super) service_owned: bool,
    pub(super) service_request_id: Option<RequestId>,
    pub(super) receiver: Option<Receiver<(OperationId, OperationResult)>>,
    pub(super) latest_operation_event: Arc<Mutex<Option<CoreOperationEvent>>>,
    pub(super) active_operation: Option<OperationEvent>,
    pub(super) operation_history: VecDeque<HistoricalOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct HistoricalOperation {
    pub(super) completed_at_unix: u64,
    pub(super) event: OperationEvent,
}

impl Default for OperationState {
    fn default() -> Self {
        Self {
            running: false,
            service_owned: false,
            service_request_id: None,
            receiver: None,
            latest_operation_event: Arc::new(Mutex::new(None)),
            active_operation: None,
            operation_history: VecDeque::new(),
        }
    }
}

impl OperationState {
    pub(super) fn restore_history(&mut self, encoded: Option<&str>) {
        let Some(history) = encoded
            .and_then(|value| serde_json::from_str::<VecDeque<HistoricalOperation>>(value).ok())
        else {
            return;
        };
        self.operation_history = history.into_iter().take(20).collect();
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

fn apply_core_operation_event(event: &CoreOperationEvent, current: &mut OperationEvent) {
    if event.id != current.id {
        return;
    }
    current.phase = event.phase;
    current.progress = event.progress.clone();
    current.status = event.status.clone();
}

impl StudioApp {
    pub(super) fn poll_operation(&mut self) {
        if self.operation.service_owned {
            return;
        }
        if !self.operation.running {
            self.poll_workshop_download_progress();
        }
        let latest_event = self
            .operation
            .latest_operation_event
            .lock()
            .ok()
            .and_then(|mut event| event.take());
        if let (Some(event), Some(active)) = (
            latest_event.as_ref(),
            self.operation.active_operation.as_mut(),
        ) {
            apply_core_operation_event(event, active);
        }
        let received = self
            .operation
            .receiver
            .as_ref()
            .map(|receiver| receiver.try_recv());
        let result = match received {
            Some(Ok(result)) => Some(result),
            Some(Err(TryRecvError::Disconnected)) => {
                self.operation.active_operation.as_ref().map(|operation| {
                    (
                        operation.id,
                        OperationResult::Message(Err(
                            "The background operation stopped before returning a result."
                                .to_owned(),
                        )),
                    )
                })
            }
            Some(Err(TryRecvError::Empty)) | None => None,
        };
        if let Some((operation_id, result)) = result {
            self.operation.receiver = None;
            self.operation.running = false;
            let failure = result.failure();
            self.apply_operation_result(result);
            if let Some(event) = self
                .operation
                .active_operation
                .as_mut()
                .filter(|event| event.id == operation_id)
            {
                event.phase = OperationPhase::Finalizing;
                event.status = match failure {
                    Some(message) => OperationStatus::Failed { message },
                    None => OperationStatus::Succeeded {
                        summary: self.status.clone(),
                    },
                };
                let completed_at_unix = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs());
                self.operation
                    .operation_history
                    .push_front(HistoricalOperation {
                        completed_at_unix,
                        event: event.clone(),
                    });
                self.operation.operation_history.truncate(20);
            }
            self.operation.active_operation = None;
            if self.service.is_some()
                && let Err(error) = self.persist_session_state()
            {
                self.status = format!("Could not save recent activity: {error}");
            }
        }
    }

    pub(super) fn spawn(
        &mut self,
        kind: OperationKind,
        operation: impl FnOnce(vapor_core::OperationContext) -> OperationResult + Send + 'static,
    ) {
        let request = OperationRequest {
            id: OperationId::fresh(),
            composition_id: self
                .composition
                .items
                .get(self.composition.active_index)
                .map(|composition| composition.id.clone()),
            kind,
        };
        let phase = match &request.kind {
            OperationKind::Resolve
            | OperationKind::LoadComposition
            | OperationKind::LoadWorkshopMetadata
            | OperationKind::LoadRuntimeOutputs
            | OperationKind::Discover
            | OperationKind::SearchWorkshop
            | OperationKind::LoadWorkshopSubscriptions
            | OperationKind::ChangeWorkshopSubscription
            | OperationKind::OpenWorkspace
            | OperationKind::RememberWorkspace
            | OperationKind::AdoptContentProject
            | OperationKind::UnregisterContentProject
            | OperationKind::DiscoverWorkspace => OperationPhase::Resolving,
            OperationKind::CreateWorkspace => OperationPhase::Building,
            OperationKind::CreateWorkspaceFile
            | OperationKind::SaveComposition
            | OperationKind::SaveWorkshopMetadata
            | OperationKind::SaveRuntimeOutputs => OperationPhase::Building,
            OperationKind::Acquire => OperationPhase::Acquiring,
            OperationKind::Install | OperationKind::Update | OperationKind::Repair => {
                OperationPhase::Installing
            }
            OperationKind::Verify | OperationKind::Validate => OperationPhase::Verifying,
            OperationKind::Build
            | OperationKind::Check
            | OperationKind::Test
            | OperationKind::Fmt
            | OperationKind::Clean => OperationPhase::Building,
            OperationKind::Run | OperationKind::RunInstalled => OperationPhase::Running,
            OperationKind::StageWorkshop | OperationKind::StageRootDistribution => {
                OperationPhase::Staging
            }
            OperationKind::PublishWorkshop => OperationPhase::Publishing,
            OperationKind::PublishRootDistribution => OperationPhase::Publishing,
            _ => OperationPhase::Queued,
        };
        self.operation.active_operation = Some(OperationEvent {
            id: request.id,
            composition_id: request.composition_id.clone(),
            kind: request.kind.clone(),
            phase,
            progress: None,
            status: OperationStatus::Running,
        });
        let operation_id = request.id;
        let initial_phase = phase;
        let core_request = CoreOperationRequest {
            id: request.id,
            domain: "studio".to_owned(),
            action: request.kind.action().to_owned(),
        };
        if let Ok(mut latest) = self.operation.latest_operation_event.lock() {
            *latest = None;
        }
        let latest_event = Arc::clone(&self.operation.latest_operation_event);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let sink: vapor_core::OperationEventSink = Arc::new(move |event| {
                if let Ok(mut latest) = latest_event.lock() {
                    *latest = Some(event);
                }
            });
            let result = OperationService::run_observed(
                core_request,
                sink,
                |context| {
                    context.set_phase(initial_phase);
                    operation(context)
                },
                |result| match result.failure() {
                    Some(message) => Err(message),
                    None => Ok("Completed".to_owned()),
                },
            );
            let _ = sender.send((operation_id, result));
        });
        self.operation.receiver = Some(receiver);
        self.operation.running = true;
    }

    pub(super) fn start_service_composition_operation(
        &mut self,
        action: ClientAction,
        kind: OperationKind,
        phase: OperationPhase,
        status: &str,
    ) {
        if self.operation.running {
            self.status = "Wait for the current operation to finish.".to_owned();
            return;
        }
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
        if self.operation.running {
            self.status = "Wait for the current operation to finish.".to_owned();
            return;
        }
        let Some(service) = self.service.as_mut() else {
            self.status = self
                .service_error
                .clone()
                .unwrap_or_else(|| "Studio service is not connected.".to_owned());
            return;
        };
        let operation_id = OperationId::fresh();
        let request_id = match service.send(action, None) {
            Ok(request_id) => request_id,
            Err(error) => {
                self.status = format!("Could not start the Studio operation: {error}");
                return;
            }
        };
        self.operation.active_operation = Some(OperationEvent {
            id: operation_id,
            composition_id,
            kind,
            phase,
            progress: None,
            status: OperationStatus::Running,
        });
        self.operation.running = true;
        self.operation.service_owned = true;
        self.operation.service_request_id = Some(request_id);
        self.status = status.to_owned();
    }

    pub(super) fn apply_service_operation_event(&mut self, event: &ServiceOperationEvent) {
        let kind = operation_kind_for_action(&event.action);
        let id = event
            .id
            .parse::<u64>()
            .map(OperationId)
            .unwrap_or_else(|_| {
                self.operation
                    .active_operation
                    .as_ref()
                    .map_or_else(OperationId::fresh, |active| active.id)
            });
        let (completed_event, terminal, status_message) = {
            let operation = self
                .operation
                .active_operation
                .get_or_insert_with(|| OperationEvent {
                    id,
                    composition_id: event.composition_id.clone(),
                    kind: kind.clone(),
                    phase: OperationPhase::Queued,
                    progress: None,
                    status: OperationStatus::Running,
                });
            operation.id = id;
            operation.composition_id = event.composition_id.clone();
            operation.kind = kind;
            operation.phase = service_phase(event.phase);
            operation.progress =
                event
                    .progress
                    .map(|progress| vapor_client_application::Progress {
                        completed: (progress.clamp(0.0, 1.0) * 100.0).round() as u64,
                        total: Some(100),
                        unit: "percent".to_owned(),
                    });
            operation.status = match &event.status {
                ServiceOperationStatus::Running => OperationStatus::Running,
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
            (operation.clone(), terminal, event.message.clone())
        };
        let awaiting_workspace_result = terminal
            && event.action.starts_with("workspace.")
            && matches!(&event.status, ServiceOperationStatus::Succeeded);
        let awaiting_authoring_result = terminal
            && event.action.starts_with("authoring.")
            && matches!(&event.status, ServiceOperationStatus::Succeeded);
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
            self.operation.active_operation = awaiting_result.then_some(completed_event);
            self.operation.running = awaiting_result;
            self.operation.service_owned = awaiting_result;
            self.operation.service_request_id = None;
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
                self.checkpoint_service_local_state();
            }
        } else {
            self.operation.running = true;
            self.operation.service_owned = true;
        }
    }

    pub(super) fn finish_service_result_operation(&mut self, operation_id: &str) {
        if !self
            .operation
            .active_operation
            .as_ref()
            .is_some_and(|operation| operation.id.0.to_string() == operation_id)
        {
            return;
        }
        self.operation.active_operation = None;
        self.operation.running = false;
        self.operation.service_owned = false;
        self.operation.service_request_id = None;
        self.checkpoint_service_local_state();
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
        operation: Option<&vapor_client_protocol::OperationSummary>,
    ) {
        if !self.operation.service_owned {
            return;
        }
        let Some(operation) = operation else {
            return;
        };
        let id = operation
            .id
            .parse::<u64>()
            .map(OperationId)
            .unwrap_or_else(|_| {
                self.operation
                    .active_operation
                    .as_ref()
                    .map_or_else(OperationId::fresh, |active| active.id)
            });
        let active = self
            .operation
            .active_operation
            .get_or_insert_with(|| OperationEvent {
                id,
                composition_id: operation.composition_id.clone(),
                kind: operation_kind_for_action(&operation.action),
                phase: service_phase(operation.phase),
                progress: None,
                status: OperationStatus::Running,
            });
        active.id = id;
        active.composition_id = operation.composition_id.clone();
        active.kind = operation_kind_for_action(&operation.action);
        active.phase = service_phase(operation.phase);
        active.progress = operation
            .progress
            .map(|progress| vapor_client_application::Progress {
                completed: (progress.clamp(0.0, 1.0) * 100.0).round() as u64,
                total: Some(100),
                unit: "percent".to_owned(),
            });
        active.status = match &operation.status {
            ServiceOperationStatus::Running => OperationStatus::Running,
            ServiceOperationStatus::Succeeded => OperationStatus::Succeeded {
                summary: "Completed".to_owned(),
            },
            ServiceOperationStatus::Failed { message } => OperationStatus::Failed {
                message: message.clone(),
            },
            ServiceOperationStatus::Cancelled => OperationStatus::Cancelled,
        };
        self.operation.running = true;
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
        "composition.preview" => OperationKind::Resolve,
        "composition.install" => OperationKind::Install,
        "composition.verify" => OperationKind::Verify,
        "composition.repair" => OperationKind::Repair,
        "composition.update" => OperationKind::Update,
        "composition.launch" => OperationKind::RunInstalled,
        "composition.run-source" => OperationKind::Run,
        "workspace.validate" => OperationKind::Validate,
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
