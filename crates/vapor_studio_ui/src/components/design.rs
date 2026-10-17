use eframe::egui;
use vapor_studio_core::InstallationState;

pub(crate) const ACCENT: egui::Color32 = egui::Color32::from_rgb(112, 190, 213);
pub(crate) const ACCENT_FILL: egui::Color32 = egui::Color32::from_rgb(34, 92, 116);
pub(crate) const MUTED_TEXT: egui::Color32 = egui::Color32::from_rgb(157, 174, 190);

/// A compact shared heading pattern for first-level pages.
pub(crate) fn page_header(ui: &mut egui::Ui, eyebrow: &str, title: &str, description: &str) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(eyebrow.to_ascii_uppercase())
                .small()
                .strong()
                .color(ACCENT),
        );
        ui.heading(title);
        ui.label(egui::RichText::new(description).color(MUTED_TEXT));
    });
    ui.add_space(12.0);
}

/// A shared heading for a section inside a page.
pub(crate) fn section_header(ui: &mut egui::Ui, title: &str, description: &str) {
    ui.add_space(6.0);
    ui.strong(egui::RichText::new(title).size(17.0));
    if !description.is_empty() {
        ui.label(egui::RichText::new(description).color(MUTED_TEXT));
    }
    ui.add_space(5.0);
}

/// A consistent, actionable empty state rather than an orphaned hint line.
pub(crate) fn empty_state(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    action: impl FnOnce(&mut egui::Ui),
) {
    let style = ui.style().clone();
    egui::Frame::group(&style).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.add_space(8.0);
        ui.strong(egui::RichText::new(title).size(18.0));
        ui.label(description);
        ui.add_space(5.0);
        action(ui);
        ui.add_space(8.0);
    });
}

pub(crate) fn confirmation_card(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    confirm_label: &str,
    cancel_label: &str,
    confirm_enabled: bool,
) -> Option<bool> {
    let style = ui.style().clone();
    egui::Frame::group(&style)
        .show(ui, |ui| {
            ui.colored_label(egui::Color32::YELLOW, title);
            ui.label(description);
            let mut decision = None;
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(confirm_enabled, egui::Button::new(confirm_label))
                    .clicked()
                {
                    decision = Some(true);
                }
                if ui.button(cancel_label).clicked() {
                    decision = Some(false);
                }
            });
            decision
        })
        .inner
}

pub(crate) fn discard_confirmation(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
) -> Option<bool> {
    confirmation_card(
        ui,
        title,
        description,
        "Discard changes",
        "Keep editing",
        true,
    )
}

pub(crate) fn status_text(ui: &mut egui::Ui, text: &str, tone: StatusTone) {
    let color = match tone {
        StatusTone::Neutral => egui::Color32::from_rgb(177, 190, 204),
        StatusTone::Positive => egui::Color32::from_rgb(118, 207, 164),
        StatusTone::Caution => egui::Color32::from_rgb(242, 193, 91),
        StatusTone::Error => egui::Color32::from_rgb(248, 117, 117),
    };
    ui.colored_label(color, text);
}

/// A single, wide call to action for the most likely next step on a screen.
pub(crate) fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let width = ui.available_width().max(1.0);
    ui.add_sized(
        [width, 38.0],
        egui::Button::new(egui::RichText::new(label).strong()).fill(ACCENT_FILL),
    )
}

#[derive(Clone, Copy)]
pub(crate) enum StatusTone {
    Neutral,
    Positive,
    Caution,
    Error,
}

/// The same installation state should mean the same thing throughout Studio.
pub(crate) fn installation_status(state: &InstallationState) -> (&'static str, StatusTone) {
    match state {
        InstallationState::Unknown => ("Status not checked", StatusTone::Neutral),
        InstallationState::NotInstalled => ("Not installed", StatusTone::Neutral),
        InstallationState::Installed { .. } => ("Installed", StatusTone::Positive),
        InstallationState::OutOfDate { .. } => ("Content differs", StatusTone::Caution),
        InstallationState::Partial { .. } | InstallationState::NeedsRepair { .. } => {
            ("Needs repair", StatusTone::Error)
        }
    }
}
