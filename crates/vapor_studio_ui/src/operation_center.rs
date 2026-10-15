use eframe::egui;
use super::StudioApp;

impl StudioApp {
    pub(super) fn draw_operation_center(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("studio-operation-center").show(ui, |ui| {
            egui::CollapsingHeader::new(if self.running { "Operation running" } else { "Operations and activity" })
                .default_open(self.running)
                .show(ui, |ui| {
                    if let Some(operation) = &self.active_operation {
                        ui.horizontal(|ui| {
                            if self.running { ui.spinner(); }
                            ui.strong(operation_kind_label(&operation.kind));
                            ui.label(operation_phase_label(&operation.phase));
                            ui.label(&self.status);
                        });
                        if let Some(progress) = &operation.progress {
                            if let Some(total) = progress.total {
                                let fraction = if total == 0 { 0.0 } else {
                                    (progress.completed as f32 / total as f32).clamp(0.0, 1.0)
                                };
                                ui.add(egui::ProgressBar::new(fraction).text(format!(
                                    "{} / {} {}", progress.completed, total, progress.unit
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
                    for operation in self.operation_history.iter().take(5) {
                        ui.horizontal(|ui| {
                            ui.small(operation_kind_label(&operation.kind));
                            ui.small(operation_status_label(&operation.status));
                        });
                    }
                });
        });
    }
}

fn operation_kind_label(kind: &vapor_studio_core::OperationKind) -> &'static str {
    use vapor_studio_core::OperationKind::*;
    match kind {
        Discover => "Refresh content catalog",
        Resolve => "Resolve composition",
        Acquire => "Download Workshop content",
        Install => "Install composition",
        Verify => "Verify installation",
        Update => "Update composition",
        Repair => "Repair installation",
        CreateWorkspace => "Create starter workspace",
        CreateContent => "Create content project",
        CreateWorkspaceFile => "Create workspace file",
        OpenWorkspace => "Open workspace",
        DiscoverWorkspace => "Read workspace files",
        OpenWorkspaceFile => "Open file",
        SaveWorkspaceFile => "Save file",
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
        Finished => "Finished",
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
