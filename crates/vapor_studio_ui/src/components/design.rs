use eframe::egui;
use vapor_client_application::InstallationState;

pub(crate) const ACCENT: egui::Color32 = egui::Color32::from_rgb(112, 190, 213);
pub(crate) const ACCENT_FILL: egui::Color32 = egui::Color32::from_rgb(34, 92, 116);
pub(crate) const MUTED_TEXT: egui::Color32 = egui::Color32::from_rgb(157, 174, 190);
pub(crate) const FORM_GRID_MIN_WIDTH: f32 = 640.0;
pub(crate) const LIST_ROW_HEIGHT: f32 = 34.0;
pub(crate) const SEARCH_FIELD_MAX_WIDTH: f32 = 360.0;
pub(crate) const CONTEXT_PANEL_WIDTH: f32 = 440.0;
const MIN_PRIMARY_PANEL_WIDTH: f32 = 480.0;

pub(crate) fn use_stacked_panels(ui: &egui::Ui) -> bool {
    let minimum_width = MIN_PRIMARY_PANEL_WIDTH
        + CONTEXT_PANEL_WIDTH
        + ui.spacing().item_spacing.x;
    ui.available_width() < minimum_width
}

pub(crate) fn use_stacked_form(ui: &egui::Ui) -> bool {
    ui.available_width() < FORM_GRID_MIN_WIDTH
}

pub(crate) fn contextual_split(
    ui: &mut egui::Ui,
    contents: impl FnOnce(&mut egui::Ui, &mut egui::Ui),
) {
    let available = ui.available_width();
    let gap = ui.spacing().item_spacing.x;
    let context_width = CONTEXT_PANEL_WIDTH.min((available - gap - 1.0).max(1.0));
    let main_width = (available - context_width - gap).max(1.0);
    let start = ui.next_widget_position();
    let available_rect = ui.available_rect_before_wrap();
    let main_rect =
        egui::Rect::from_min_size(start, egui::vec2(main_width, available_rect.height()));
    let context_rect = egui::Rect::from_min_size(
        egui::pos2(start.x + main_width + gap, start.y),
        egui::vec2(context_width, available_rect.height()),
    );
    let layout = egui::Layout::top_down_justified(egui::Align::Min);
    let mut main_ui = ui.new_child(egui::UiBuilder::new().max_rect(main_rect).layout(layout));
    let mut context_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(context_rect)
            .layout(egui::Layout::top_down_justified(egui::Align::Min)),
    );
    let style = context_ui.style().clone();
    egui::Frame::group(&style).show(&mut context_ui, |panel| {
        contents(&mut main_ui, panel);
    });
    let bottom = main_ui
        .min_rect()
        .bottom()
        .max(context_ui.min_rect().bottom());
    ui.advance_cursor_after_rect(egui::Rect::from_min_max(
        start,
        egui::pos2(available_rect.right(), bottom),
    ));
}

pub(crate) fn search_field_width(ui: &egui::Ui) -> f32 {
    ui.available_width().min(SEARCH_FIELD_MAX_WIDTH)
}

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
            status_text(ui, title, StatusTone::Caution);
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

pub(crate) fn status_text(ui: &mut egui::Ui, text: &str, tone: StatusTone) -> egui::Response {
    ui.colored_label(status_color(tone), text)
}

fn status_color(tone: StatusTone) -> egui::Color32 {
    match tone {
        StatusTone::Neutral => egui::Color32::from_rgb(177, 190, 204),
        StatusTone::Positive => egui::Color32::from_rgb(118, 207, 164),
        StatusTone::Caution => egui::Color32::from_rgb(242, 193, 91),
        StatusTone::Error => egui::Color32::from_rgb(248, 117, 117),
    }
}

/// A single, wide call to action for the most likely next step on a screen.
pub(crate) fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let width = ui.available_width().clamp(1.0, 360.0);
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
