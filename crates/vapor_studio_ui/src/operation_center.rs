use super::StudioApp;
use super::components::design::{StatusTone, status_text};
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_operation_center(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if let Some(operation) = &self.operation.active_operation {
                if self.operation.running {
                    ui.spinner();
                }
                ui.strong(operation_kind_label(&operation.kind));
                ui.weak(operation_phase_label(&operation.phase));
                ui.label(&self.status);
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
                    vapor_studio_core::OperationStatus::Succeeded { summary }
                        if summary == &self.status =>
                    {
                        StatusTone::Positive
                    }
                    vapor_studio_core::OperationStatus::Failed { message }
                        if message == &self.status =>
                    {
                        StatusTone::Error
                    }
                    vapor_studio_core::OperationStatus::Cancelled if self.status == "Cancelled" => {
                        StatusTone::Caution
                    }
                    _ => StatusTone::Neutral,
                };
                status_text(ui, &self.status, tone);
                ui.weak(format!(
                    "Last · {} · {}",
                    operation_kind_label(&last.event.kind),
                    super::operations::elapsed_label(last.completed_at_unix),
                ));
            } else {
                ui.strong("Ready");
                status_text(ui, &self.status, StatusTone::Neutral);
            }
        });
        egui::CollapsingHeader::new("Recent activity")
            .default_open(false)
            .show(ui, |ui| {
                if self.operation.operation_history.is_empty() {
                    ui.weak("No operations have been recorded yet.");
                } else {
                    for operation in self.operation.operation_history.iter().take(10) {
                        ui.horizontal(|ui| {
                            ui.strong(operation_kind_label(&operation.event.kind));
                            ui.label(operation_status_label(&operation.event.status));
                            ui.weak(super::operations::elapsed_label(
                                operation.completed_at_unix,
                            ));
                        });
                    }
                }
            });
    }
}

pub(super) fn operation_kind_label(kind: &vapor_studio_core::OperationKind) -> &'static str {
    use vapor_studio_core::OperationKind::*;
    match kind {
        Discover => "Refresh content catalog",
        SearchWorkshop => "Search Steam Workshop",
        LoadWorkshopSubscriptions => "Load Workshop subscriptions",
        ChangeWorkshopSubscription => "Change Workshop subscriptions",
        Resolve => "Resolve composition",
        Acquire => "Download Workshop content",
        Install => "Install composition",
        Verify => "Verify installation",
        Update => "Update composition",
        Repair => "Repair installation",
        CreateWorkspace => "Create starter workspace",
        CreateContent => "Create content project",
        LoadComposition => "Load composition dependencies",
        SaveComposition => "Save composition dependencies",
        LoadWorkshopMetadata => "Load Workshop metadata",
        SaveWorkshopMetadata => "Save Workshop metadata",
        LoadRuntimeOutputs => "Load runtime output declarations",
        SaveRuntimeOutputs => "Save runtime output declarations",
        CreateWorkspaceFile => "Create workspace file",
        OpenWorkspace => "Open workspace",
        RememberWorkspace => "Add workspace to Vapor sources",
        AdoptContentProject => "Adopt existing content project",
        UnregisterContentProject => "Unregister content project",
        DiscoverContentProjects => "Find unregistered content",
        DiscoverWorkspace => "Read workspace files",
        OpenWorkspaceFile => "Open file",
        SaveWorkspaceFile => "Save workspace file(s)",
        Fmt => "Format workspace",
        Check => "Check workspace",
        Test => "Test workspace",
        Build => "Build workspace",
        Clean => "Clean workspace",
        Validate => "Validate content",
        Run => "Build and play composition",
        RunInstalled => "Play installed composition",
        StageWorkshop => "Stage Workshop review",
        PublishWorkshop => "Publish to Workshop",
        StageRootDistribution => "Preview Steam package",
        PublishRootDistribution => "Publish Steam root package",
    }
}

fn operation_phase_label(phase: &vapor_studio_core::OperationPhase) -> &'static str {
    use vapor_studio_core::OperationPhase::*;
    match phase {
        Queued => "Queued",
        Resolving => "Resolving",
        Acquiring => "Downloading",
        Installing => "Installing",
        Verifying => "Verifying",
        Building => "Building",
        Running => "Launching",
        Staging => "Preparing review",
        Publishing => "Publishing",
        Finalizing => "Finishing",
    }
}

fn operation_status_label(status: &vapor_studio_core::OperationStatus) -> String {
    use vapor_studio_core::OperationStatus::*;
    match status {
        Running => "In progress".to_owned(),
        Succeeded { summary } => format!("Complete · {summary}"),
        Failed { message } => format!("Failed · {message}"),
        Cancelled => "Cancelled".to_owned(),
    }
}
