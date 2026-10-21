use super::StudioApp;
use super::components::design::{StatusTone, status_text};
use eframe::egui;
use vapor_client_protocol::ClientAction;

impl StudioApp {
    pub(super) fn show_console_viewport(&mut self, context: &egui::Context) {
        if !self.console_open {
            return;
        }

        let viewport_id = egui::ViewportId::from_hash_of("vapor-studio-console");
        let close_requested = context.show_viewport_immediate(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("Vapor Studio — Console")
                .with_inner_size([820.0, 560.0])
                .with_min_inner_size([480.0, 260.0])
                .with_resizable(true)
                .with_transparent(false),
            |ui, _class| {
                let close_requested = ui.input(|input| input.viewport().close_requested());
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(egui::Color32::BLACK))
                    .show(ui, |ui| self.draw_operation_console(ui));
                close_requested || !self.console_open
            },
        );
        if close_requested {
            self.console_open = false;
        }
    }

    pub(super) fn has_failed_operation(&self) -> bool {
        self.operation
            .active_operation
            .as_ref()
            .is_some_and(|operation| {
                matches!(
                    &operation.status,
                    vapor_client_application::OperationStatus::Failed { .. }
                )
            })
            || (!self.operation.running
                && self
                    .operation
                    .operation_history
                    .front()
                    .is_some_and(|operation| {
                        matches!(
                            &operation.event.status,
                            vapor_client_application::OperationStatus::Failed { .. }
                        )
                    }))
    }

    pub(super) fn draw_operation_center(&mut self, ui: &mut egui::Ui) {
        let mut cancel_operation_id = None;
        ui.horizontal_wrapped(|ui| {
            if let Some(operation) = &self.operation.active_operation {
                if self.operation.running {
                    ui.spinner();
                }
                ui.strong(operation_kind_label(&operation.kind));
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
    }

    pub(super) fn draw_operation_console(&mut self, ui: &mut egui::Ui) {
        let mut close_console = false;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Vapor Console")
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            if self.operation.dropped_output_lines == 0 {
                ui.weak(format!("{} lines", self.operation.output_lines.len()));
            } else {
                ui.weak(format!(
                    "Showing latest {} lines · {} earlier lines omitted",
                    self.operation.output_lines.len(),
                    self.operation.dropped_output_lines
                ));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close_console = ui.small_button("Close").clicked();
            });
        });
        if close_console {
            self.console_open = false;
        }
        ui.separator();
        if self.operation.dropped_output_lines > 0 {
            ui.colored_label(
                egui::Color32::YELLOW,
                format!(
                    "Earlier output was omitted to keep memory bounded ({} lines).",
                    self.operation.dropped_output_lines
                ),
            );
        }
        let available_height = ui.available_height();
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .max_height(available_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.status.chars().count() > 120 || self.has_failed_operation() {
                    ui.label(
                        egui::RichText::new("Latest message")
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&self.status)
                                .monospace()
                                .color(egui::Color32::LIGHT_GRAY),
                        )
                        .wrap_mode(egui::TextWrapMode::Extend),
                    );
                    if !self.operation.output_lines.is_empty() {
                        ui.separator();
                    }
                }
                for line in &self.operation.output_lines {
                    let (label, color) = match line.stream {
                        vapor_client_protocol::OperationOutputStream::Stdout => {
                            ("OUT", egui::Color32::from_rgb(130, 205, 150))
                        }
                        vapor_client_protocol::OperationOutputStream::Stderr => {
                            ("ERR", egui::Color32::from_rgb(255, 130, 130))
                        }
                    };
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(label).monospace().color(color));
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&line.text)
                                    .monospace()
                                    .color(egui::Color32::from_rgb(230, 233, 238)),
                            )
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    });
                }
            });
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
