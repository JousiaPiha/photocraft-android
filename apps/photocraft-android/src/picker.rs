//! Nonblocking document picker for Android's app-owned shared Documents folder.
use photocraft_ui_egui::{FileDialogAnswer, FileDialogReply, FileDialogRequest};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

pub type Request = (FileDialogRequest, FileDialogReply);

pub struct Picker {
    root: PathBuf,
    requests: Receiver<Request>,
    pending: Option<Request>,
    files: Vec<String>,
    name: String,
    error: String,
    overwrite: bool,
    selected: std::collections::BTreeSet<String>,
}

impl Picker {
    pub fn new(root: PathBuf) -> (Self, Sender<Request>) {
        let (tx, requests) = mpsc::channel();
        (
            Self {
                root,
                requests,
                pending: None,
                files: Vec::new(),
                name: String::new(),
                error: String::new(),
                overwrite: false,
                selected: Default::default(),
            },
            tx,
        )
    }

    fn refresh(&mut self) {
        self.files.clear();
        match std::fs::read_dir(&self.root) {
            Ok(entries) => {
                // Bound the list, including directories and invalid entries.
                for entry in entries.take(4096).flatten() {
                    if entry.file_type().is_ok_and(|t| t.is_file()) {
                        self.files.push(entry.file_name().to_string_lossy().into_owned());
                    }
                }
                self.files.sort_unstable();
            }
            Err(e) => self.error = e.to_string(),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if self.pending.is_none()
            && let Ok(request) = self.requests.try_recv()
        {
            self.name = match &request.0 {
                FileDialogRequest::Save { suggested } => std::path::Path::new(suggested).file_name().unwrap_or_default().to_string_lossy().into_owned(),
                _ => String::new(),
            };
            self.error.clear();
            self.overwrite = false;
            self.selected.clear();
            self.pending = Some(request);
            self.refresh();
        }
        let Some((request, _)) = &self.pending else { return };
        let save = matches!(request, FileDialogRequest::Save { .. });
        let multiple = matches!(request, FileDialogRequest::Open { multiple: true, .. });
        let folder = matches!(request, FileDialogRequest::Folder { .. });
        let extensions = match request {
            FileDialogRequest::Open { extensions, .. } => extensions.clone(),
            _ => None,
        };
        let mut result = None;
        let modal = egui::Modal::new(egui::Id::new("android_files")).show(ctx, |ui| {
            ui.set_width(440.0_f32.min(ctx.content_rect().width() - 48.0).max(100.0));
            ui.heading(if save { "Save document" } else { "Open document" });
            ui.label("PhotoCraft Documents");
            ui.small("Copy images here using a file manager or USB:");
            ui.small(self.root.display().to_string());
            ui.separator();
            if !folder {
                egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 240.0).clamp(36.0, 320.0)).show(ui, |ui| {
                    if self.files.is_empty() {
                        ui.label("This folder is empty.");
                    }
                    for name in &self.files {
                        if !save
                            && extensions.as_ref().is_some_and(|exts| {
                                !exts.iter().any(|ext| std::path::Path::new(name).extension().is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(ext)))
                            })
                        {
                            continue;
                        }
                        if ui.add_sized([ui.available_width(), 36.0], egui::Button::new(name).selected(self.selected.contains(name))).clicked() {
                            if save {
                                self.name = name.clone();
                                self.overwrite = false;
                            } else if multiple {
                                if !self.selected.remove(name) {
                                    self.selected.insert(name.clone());
                                }
                            } else {
                                result = Some(Some(FileDialogAnswer::Paths(vec![self.root.join(name).to_string_lossy().into_owned()])));
                            }
                        }
                    }
                });
            }
            if save {
                ui.label("File name (.pcraft, .psd, .png, .jpg, …)");
                if ui.text_edit_singleline(&mut self.name).changed() {
                    self.overwrite = false;
                }
            }
            if !self.error.is_empty() {
                ui.colored_label(photocraft_ui_egui::theme::Tokens::get(ctx).danger, &self.error);
            }
            if self.overwrite {
                ui.label("This file already exists. Replace it?");
            }
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    result = Some(None);
                }
                if ui.button("Refresh").clicked() {
                    self.refresh();
                }
                if multiple && ui.add_enabled(!self.selected.is_empty(), egui::Button::new("Open selected")).clicked() {
                    result =
                        Some(Some(FileDialogAnswer::Paths(self.selected.iter().map(|name| self.root.join(name).to_string_lossy().into_owned()).collect())));
                }
                if folder && ui.button("Choose folder").clicked() {
                    result = Some(Some(FileDialogAnswer::FolderTo(self.root.to_string_lossy().into_owned())));
                }
                if save && ui.button(if self.overwrite { "Replace" } else { "Save" }).clicked() {
                    match crate::storage::save_path(&self.root, &self.name) {
                        Ok(path) if path.exists() && !self.overwrite => self.overwrite = true,
                        Ok(path) => result = Some(Some(FileDialogAnswer::SaveTo(path.to_string_lossy().into_owned()))),
                        Err(error) => self.error = error,
                    }
                }
            });
        });
        if modal.should_close() {
            result = Some(None);
        }
        if let Some(answer) = result
            && let Some((_, reply)) = self.pending.take()
        {
            reply.send(answer);
        }
    }
}
