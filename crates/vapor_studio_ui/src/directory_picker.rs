use eframe::egui;
use std::{fs, path::{Path, PathBuf}};

#[derive(Clone, Copy)]
pub(super) enum DirectoryPickerPurpose {
    OpenWorkspace,
    WorkspaceParent,
}

pub(super) enum DirectoryPickerResult {
    Cancelled,
    Selected { purpose: DirectoryPickerPurpose, path: PathBuf },
}

pub(super) struct DirectoryPicker {
    purpose: DirectoryPickerPurpose,
    path: PathBuf,
    path_draft: String,
    directories: Vec<PathBuf>,
    overflow: bool,
    error: Option<String>,
    open: bool,
}

impl DirectoryPicker {
    pub(super) fn new(purpose: DirectoryPickerPurpose, initial: &Path) -> Self {
        let starting_path = fs::canonicalize(initial)
            .ok()
            .filter(|path| path.is_dir())
            .or_else(|| {
                initial
                    .parent()
                    .and_then(|parent| fs::canonicalize(parent).ok())
                    .filter(|path| path.is_dir())
            })
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let mut picker = Self {
            purpose,
            path: PathBuf::new(),
            path_draft: String::new(),
            directories: Vec::new(),
            overflow: false,
            error: None,
            open: true,
        };
        picker.navigate(&starting_path);
        picker
    }

    fn navigate(&mut self, requested: &Path) {
        match fs::canonicalize(requested) {
            Ok(path) if path.is_dir() => {
                self.path = path;
                self.path_draft = self.path.display().to_string();
                self.error = None;
            }
            Ok(path) => {
                self.error = Some(format!("{} is not a folder.", path.display()));
                return;
            }
            Err(error) => {
                self.error = Some(format!("Could not open {}: {error}", requested.display()));
                return;
            }
        }

        let mut directories = Vec::new();
        let mut overflow = false;
        match fs::read_dir(&self.path) {
            Ok(entries) => {
                for entry in entries {
                    let Ok(entry) = entry else { continue };
                    let Ok(kind) = entry.file_type() else { continue };
                    // Do not follow symlinks: the chooser is a bounded directory browser.
                    if !kind.is_dir() {
                        continue;
                    }
                    if directories.len() == 500 {
                        overflow = true;
                        break;
                    }
                    directories.push(entry.path());
                }
                directories.sort_by_key(|path| {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                });
            }
            Err(error) => {
                self.error = Some(format!("Could not list {}: {error}", self.path.display()));
            }
        }
        self.directories = directories;
        self.overflow = overflow;
    }

    pub(super) fn show(&mut self, context: &egui::Context) -> Option<DirectoryPickerResult> {
        let title = match self.purpose {
            DirectoryPickerPurpose::OpenWorkspace => "Open a workspace folder",
            DirectoryPickerPurpose::WorkspaceParent => "Choose where to create the workspace",
        };
        let mut window_open = self.open;
        let mut result = None;
        egui::Window::new(title)
            .id(egui::Id::new("studio-directory-picker"))
            .open(&mut window_open)
            .collapsible(false)
            .resizable(true)
            .default_width(620.0)
            .show(context, |ui| {
                ui.label(match self.purpose {
                    DirectoryPickerPurpose::OpenWorkspace => {
                        "Choose a folder containing Workspace.vapor.toml."
                    }
                    DirectoryPickerPurpose::WorkspaceParent => {
                        "Studio will create a new workspace folder inside this location."
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    let up = self.path.parent().map(Path::to_path_buf);
                    if ui
                        .add_enabled(up.is_some(), egui::Button::new("↑ Up"))
                        .clicked()
                    {
                        if let Some(up) = up {
                            self.navigate(&up);
                        }
                    }
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.path_draft)
                            .desired_width(ui.available_width().max(160.0))
                            .hint_text("Folder path"),
                    );
                    let go = ui.button("Go").clicked()
                        || (response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                    if go {
                        let requested = PathBuf::from(self.path_draft.trim());
                        self.navigate(&requested);
                    }
                });

                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                let recognized_workspace = self.path.join("Workspace.vapor.toml").is_file();
                if matches!(self.purpose, DirectoryPickerPurpose::OpenWorkspace) {
                    if recognized_workspace {
                        ui.colored_label(egui::Color32::from_rgb(118, 207, 164), "Vapor workspace found here");
                    } else {
                        ui.weak("This folder does not contain a Vapor workspace manifest.");
                    }
                }
                ui.separator();
                ui.label(egui::RichText::new(&self.path.display().to_string()).strong());
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        if self.directories.is_empty() {
                            ui.weak("No subfolders here.");
                        }
                        let mut navigate_to = None;
                        for directory in &self.directories {
                            let name = directory
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy();
                            if ui.selectable_label(false, format!("›  {name}")).clicked() {
                                navigate_to = Some(directory.clone());
                            }
                        }
                        if self.overflow {
                            ui.weak("Showing the first 500 folders. Type a path above to narrow the view.");
                        }
                        if let Some(directory) = navigate_to {
                            self.navigate(&directory);
                        }
                    });

                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Cancel").clicked() {
                        result = Some(DirectoryPickerResult::Cancelled);
                    }
                    let can_select = match self.purpose {
                        DirectoryPickerPurpose::OpenWorkspace => {
                            self.error.is_none() && recognized_workspace
                        }
                        DirectoryPickerPurpose::WorkspaceParent => self.error.is_none(),
                    };
                    let label = match self.purpose {
                        DirectoryPickerPurpose::OpenWorkspace => "Open this workspace",
                        DirectoryPickerPurpose::WorkspaceParent => "Choose this location",
                    };
                    if ui
                        .add_enabled(can_select, egui::Button::new(label).fill(egui::Color32::from_rgb(34, 92, 116)))
                        .clicked()
                    {
                        result = Some(DirectoryPickerResult::Selected {
                            purpose: self.purpose,
                            path: self.path.clone(),
                        });
                    }
                });
            });
        self.open = window_open;
        result.or_else(|| (!self.open).then_some(DirectoryPickerResult::Cancelled))
    }
}
