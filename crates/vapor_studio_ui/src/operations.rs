use super::StudioApp;
use super::operation_protocol::OperationResult;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc::{self, Receiver, TryRecvError}},
    thread,
};
use vapor_core::{
    OperationEvent as CoreOperationEvent,
    OperationRequest as CoreOperationRequest,
    OperationService,
};
use vapor_studio_core::{
    OperationEvent, OperationId, OperationKind,
    OperationPhase, OperationRequest, OperationStatus,
};

pub(super) struct OperationState {
    pub(super) running: bool,
    pub(super) receiver: Option<Receiver<(OperationId, OperationResult)>>,
    pub(super) latest_operation_event: Arc<Mutex<Option<CoreOperationEvent>>>,
    pub(super) active_operation: Option<OperationEvent>,
    pub(super) operation_history: VecDeque<OperationEvent>,
}

impl Default for OperationState {
    fn default() -> Self {
        Self {
            running: false,
            receiver: None,
            latest_operation_event: Arc::new(Mutex::new(None)),
            active_operation: None,
            operation_history: VecDeque::new(),
        }
    }
}

fn apply_core_operation_event(
    event: &CoreOperationEvent,
    current: &mut OperationEvent,
) {
    if event.id != current.id {
        return;
    }
    current.phase = event.phase;
    current.progress = event.progress.clone();
    current.status = event.status.clone();
}

impl StudioApp {
    pub(super) fn poll_operation(&mut self) {
        let latest_event = self
            .operation
            .latest_operation_event
            .lock()
            .ok()
            .and_then(|mut event| event.take());
        if let (Some(event), Some(active)) = (latest_event.as_ref(), self.operation.active_operation.as_mut())
        {
            apply_core_operation_event(event, active);
        }
        let received = self.operation.receiver.as_ref().map(|receiver| receiver.try_recv());
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
            let refresh_content_catalog = result.refreshes_content_catalog();
            let refresh_subscriptions = result.refreshes_workshop_subscriptions();
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
                self.operation.operation_history.push_front(event.clone());
                self.operation.operation_history.truncate(20);
            }
            self.operation.active_operation = None;
            if refresh_content_catalog {
                self.refresh_content_catalog();
            }
            if refresh_subscriptions {
                self.refresh_workshop_subscriptions();
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
            profile_id: self
                .profile
                .items
                .get(self.profile.active_index)
                .map(|profile| profile.id.clone()),
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
            | OperationKind::DiscoverContentProjects
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
            _ => OperationPhase::Queued,
        };
        self.operation.active_operation = Some(OperationEvent {
            id: request.id,
            profile_id: request.profile_id.clone(),
            kind: request.kind.clone(),
            phase,
            progress: None,
            status: OperationStatus::Running,
        });
        let operation_id = request.id;
        let initial_phase = phase.clone();
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

}

