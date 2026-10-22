use super::StudioApp;
use super::components::design::{StatusTone, status_text};
use eframe::egui;
use vapor_client_protocol::ClientAction;

impl StudioApp {
    pub(super) fn draw_operation_center(&mut self, ui: &mut egui::Ui) {
        let mut cancel_operation_id = None;
        ui.horizontal_wrapped(|ui| {
            if let Some(operation) = &self.operation.active_operation {
                if self.operation.running {
                    ui.spinner();
                }
                ui.strong(operation_kind_label(&operation.kind));
                if self.operation.active_operations.len() > 1 {
                    ui.weak(format!(
                        "+ {} more",
                        self.operation.active_operations.len() - 1
                    ));
                }
                ui.weak(operation_phase_label(&operation.phase));
                ui.label(compact_status(&self.status))
                    .on_hover_text(&self.status);
                if operation.status == vapor_client_application::OperationStatus::Cancelling {
                    ui.weak("Stopping…");
                } else if self.operation.service_owned
                    && matches!(
                        &operation.status,
                        vapor_client_application::OperationStatus::Running
                    )
                    && matches!(
                        &operation.kind,
                        vapor_client_application::OperationKind::Fmt
                            | vapor_client_application::OperationKind::Check
                            | vapor_client_application::OperationKind::Test
                            | vapor_client_application::OperationKind::Build
                    )
                    && ui.button("Stop").clicked()
                {
                    cancel_operation_id = Some(operation.id.0.to_string());
                }
                if let Some(progress) = &operation.progress {
                    if let Some(message) = &progress.message {
                        ui.weak(message);
                    }
                    if let Some(total) = progress.total {
                        let fraction = if total == 0 {
                            0.0
                        } else {
                            (progress.completed as f32 / total as f32).clamp(0.0, 1.0)
                        };
                        ui.add(egui::ProgressBar::new(fraction).desired_width(150.0).text(
                            format!("{} / {} {}", progress.completed, total, progress.unit),
                        ));
                    } else {
                        ui.weak(format!("{} {}", progress.completed, progress.unit));
                    }
                }
            } else if !self.operation.pending_service_requests.is_empty() {
                ui.spinner();
                ui.strong("Sending task to Vapor…");
                ui.weak(format!(
                    "{} request{} pending",
                    self.operation.pending_service_requests.len(),
                    if self.operation.pending_service_requests.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                ));
            } else if let Some(last) = self.operation.operation_history.front() {
                let tone = match &last.event.status {
                    vapor_client_application::OperationStatus::Succeeded { summary }
                        if summary == &self.status =>
                    {
                        StatusTone::Positive
                    }
                    vapor_client_application::OperationStatus::Failed { message }
                        if message == &self.status =>
                    {
                        StatusTone::Error
                    }
                    vapor_client_application::OperationStatus::Cancelled
                        if self.status == "Cancelled" =>
                    {
                        StatusTone::Caution
                    }
                    _ => StatusTone::Neutral,
                };
                status_text(ui, &compact_status(&self.status), tone).on_hover_text(&self.status);
                ui.weak(format!(
                    "Last · {} · {}",
                    operation_kind_label(&last.event.kind),
                    super::operations::elapsed_label(last.completed_at_unix),
                ));
            } else {
                ui.strong("Ready");
                status_text(ui, &compact_status(&self.status), StatusTone::Neutral)
                    .on_hover_text(&self.status);
            }
        });
        if let Some(operation_id) = cancel_operation_id {
            self.submit_service_action(ClientAction::Session(
                vapor_client_protocol::SessionAction::CancelOperation { operation_id },
            ));
        }
        if self.console_open {
            ui.separator();
            self.draw_operation_console(ui);
        }
    }

    pub(super) fn draw_operation_console(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Vapor Console");
            ui.weak(format!("{} records", self.operation.console.history.len()));
            if ui.small_button("Hide").clicked() {
                self.console_open = false;
            }
        });
        if self.operation.console.history.dropped_count() > 0 {
            ui.weak(format!(
                "{} oldest lines omitted to keep memory bounded",
                self.operation.console.history.dropped_count()
            ));
        }
        if let Some(command) = self.operation.console.show(ui, true) {
            self.submit_console_command(&command);
        }
    }

    fn submit_console_command(&mut self, line: &str) {
        use vapor_client_domain::{
            ConsoleCommand, ConsoleCommandOutcome, ConsoleCommandScope, console_command_catalog,
        };
        use vapor_console_core::{ConsoleLevel, ConsoleRecord};
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
            .unwrap_or_default();
        self.operation.console.history.push(ConsoleRecord::new(
            now,
            ConsoleLevel::Input,
            "console",
            line,
        ));
        match ConsoleCommand::parse(line) {
            Ok(ConsoleCommand::Help) => {
                for command in console_command_catalog(ConsoleCommandScope::Workspace) {
                    self.operation.console.history.push(
                        ConsoleRecord::new(now, ConsoleLevel::Info, "help", command.path)
                            .field("description", command.summary),
                    );
                }
            }
            Ok(ConsoleCommand::Status) => {
                self.operation.console.history.push(ConsoleRecord::new(
                    now,
                    ConsoleLevel::Info,
                    "vapor",
                    self.status.clone(),
                ));
            }
            Ok(command) => match command.outcome() {
                ConsoleCommandOutcome::Action(action) => self.submit_service_action(*action),
                ConsoleCommandOutcome::Help | ConsoleCommandOutcome::Status => unreachable!(),
            },
            Err(error) => self.operation.console.history.push(ConsoleRecord::new(
                now,
                ConsoleLevel::Error,
                "console",
                format!("{error:?}"),
            )),
        }
    }
}

pub(super) fn compact_status(status: &str) -> String {
    let mut characters = status.chars();
    let mut summary = characters.by_ref().take(120).collect::<String>();
    if characters.next().is_some() {
        summary.push('…');
    }
    summary
}

pub(super) fn operation_kind_label(kind: &vapor_client_application::OperationKind) -> &'static str {
    use vapor_client_application::OperationKind::*;
    match kind {
        Discover => "Refresh available content",
        SearchWorkshop => "Search Steam Workshop",
        LoadWorkshopSubscriptions => "Check your Workshop subscriptions",
        ChangeWorkshopSubscription => "Update Workshop subscriptions",
        Resolve => "Check composition",
        Acquire => "Download content",
        Install => "Install composition",
        Verify => "Check installed files",
        Update => "Update composition",
        Repair => "Repair composition",
        CreateWorkspace => "Create project folder",
        CreateContent => "Create content project",
        ForkContent => "Fork Workshop content",
        LoadComposition => "Load included content",
        SaveComposition => "Save included content",
        LoadWorkshopMetadata => "Load Workshop metadata",
        SaveWorkshopMetadata => "Save Workshop metadata",
        LoadRuntimeOutputs => "Load runtime output declarations",
        SaveRuntimeOutputs => "Save runtime output declarations",
        PackageContent => "Package content",
        CreateWorkspaceFile => "Create project file",
        OpenWorkspace => "Open project folder",
        RememberWorkspace => "Make project folder available",
        AdoptContentProject => "Add existing project",
        UnregisterContentProject => "Remove project from Studio",
        DiscoverWorkspace => "Read project files",
        OpenWorkspaceFile => "Open file",
        SaveWorkspaceFile => "Save project files",
        Fmt => "Format project files",
        Check => "Check project",
        Test => "Test project",
        Build => "Build project",
        Clean => "Clean project files",
        Validate => "Check content setup",
        Run => "Build and launch project",
        RunInstalled => "Launch composition",
        StageWorkshop => "Review Workshop upload",
        PublishWorkshop => "Publish to Steam Workshop",
        StageRootDistribution => "Review Steam release",
        PublishRootDistribution => "Publish Steam release",
    }
}

fn operation_phase_label(phase: &vapor_client_application::OperationPhase) -> &'static str {
    use vapor_client_application::OperationPhase::*;
    match phase {
        Queued => "Queued",
        Resolving => "Checking requirements",
        Acquiring => "Downloading",
        Installing => "Installing",
        Verifying => "Checking files",
        Building => "Building",
        Running => "Launching",
        Staging => "Preparing review",
        Publishing => "Publishing",
        Finalizing => "Finishing",
    }
}
