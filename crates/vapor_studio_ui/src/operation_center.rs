use super::StudioApp;
use eframe::egui;

impl StudioApp {
    pub(super) fn draw_operation_center(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(if self.operation.running {
            "Operation running"
        } else {
            "Operations and activity"
        })
        .default_open(self.operation.running)
        .show(ui, |ui| {
            if let Some(operation) = &self.operation.active_operation {
                ui.horizontal(|ui| {
                    if self.operation.running {
                        ui.spinner();
                    }
                    ui.strong(operation_kind_label(&operation.kind));
                    ui.label(operation_phase_label(&operation.phase));
                    ui.label(&self.status);
                });
                if let Some(progress) = &operation.progress {
                    if let Some(total) = progress.total {
                        let fraction = if total == 0 {
                            0.0
                        } else {
                            (progress.completed as f32 / total as f32).clamp(0.0, 1.0)
                        };
                        ui.add(egui::ProgressBar::new(fraction).text(format!(
                            "{} / {} {}",
                            progress.completed, total, progress.unit
                        )));
                    } else {
                        ui.label(format!("{} {}", progress.completed, progress.unit));
                    }
                }
                if let Some(profile_id) = &operation.profile_id {
                    ui.small(format!("Profile {profile_id}"));
                }
            } else {
                ui.label(&self.status);
            }
            for operation in self.operation.operation_history.iter().take(5) {
                ui.horizontal(|ui| {
                    ui.small(operation_kind_label(&operation.event.kind));
                    ui.small(operation_status_label(&operation.event.status));
                    ui.weak(super::operations::elapsed_label(
                        operation.completed_at_unix,
                    ));
                });
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
        LoadComposition => "Load content dependency graph",
        SaveComposition => "Save content dependency graph",
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
