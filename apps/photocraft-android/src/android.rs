use photocraft_ui_egui::PhotocraftApp;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub fn run(android_app: android_activity::AndroidApp) -> Result<(), String> {
    let content_surface = crate::surface::install(&android_app);
    crate::crash_guard::install_hook();
    let private = android_app.internal_data_path().ok_or("Android private storage is unavailable")?;
    std::fs::create_dir_all(&private).map_err(|e| e.to_string())?;
    let documents =
        android_app.external_data_path().as_deref().and_then(|p| crate::storage::document_directory(p).ok()).unwrap_or_else(|| private.join("Documents"));
    std::fs::create_dir_all(&documents).map_err(|e| format!("Cannot create document folder: {e}"))?;
    log::info!("Documents: {}", documents.display());
    let control = control_config(&private);
    let workspace = if control.is_some() {
        Some(photocraft_automation::AuthorizedWorkspace::new(Some(&documents), Some(&documents)).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let (picker, requests) = crate::picker::Picker::new(documents);
    let (services, recovery, recovery_errors) = crate::services::native(private.clone(), requests, workspace);
    let options = eframe::NativeOptions {
        android_app: Some(android_app.clone()),
        renderer: eframe::Renderer::Wgpu,
        persistence_path: Some(private.join("ui.ron")),
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "PhotoCraft",
        options,
        Box::new(move |cc| {
            let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), services);
            PhotocraftApp::setup_context(&cc.egui_ctx, app.ui.theme);
            if let Some(rs) = &cc.wgpu_render_state {
                app.set_wgpu(rs.clone());
            }
            if let Some((port, token)) = control {
                app = app.with_control(crate::control_server::start(port, token, cc.egui_ctx.clone()));
            }
            Ok(Box::new(AndroidEditor {
                app,
                android_app,
                content_surface,
                picker,
                last_recovery: Instant::now(),
                focused: true,
                fullscreen: Default::default(),
                recovery,
                recovery_errors,
                checkpointed: Default::default(),
            }))
        }),
    )
    .map_err(|e| e.to_string())
}

/// Opt-in only: a token held in the application's private, adb run-as accessible directory.
fn control_config(private: &Path) -> Option<(u16, String)> {
    let text = crate::storage::read_text(&private.join("control.json"), 4096).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let port = u16::try_from(value.get("port")?.as_u64()?).ok().filter(|p| *p != 0)?;
    let token = value.get("token")?.as_str()?.to_owned();
    (token.len() >= 32 && token.len() <= 256).then_some((port, token))
}

struct AndroidEditor {
    app: PhotocraftApp,
    android_app: android_activity::AndroidApp,
    content_surface: std::sync::Arc<std::sync::atomic::AtomicBool>,
    picker: crate::picker::Picker,
    last_recovery: Instant,
    focused: bool,
    fullscreen: crate::fullscreen::Fullscreen,
    recovery: std::rc::Rc<std::cell::RefCell<crate::recovery::Recovery>>,
    recovery_errors: std::sync::mpsc::Receiver<String>,
    checkpointed: std::collections::HashSet<u64>,
}

impl AndroidEditor {
    fn checkpoint(&mut self) {
        let dirty: std::collections::HashSet<u64> = self.app.session.documents().iter().filter(|s| s.is_dirty()).map(|s| s.doc.id.0).collect();
        for id in self.checkpointed.difference(&dirty) {
            if let Some(discard) = &mut self.app.services.discard_autosave {
                discard(*id);
            }
        }
        self.checkpointed.retain(|id| dirty.contains(id));
        if let Some(save) = &mut self.app.services.autosave {
            for state in self.app.session.documents().iter().filter(|s| s.is_dirty()) {
                if let Err(error) = save(&state.doc, state.revision, state.path.as_deref()) {
                    self.app.ui.status_error = true;
                    self.app.ui.status = format!("Recovery save failed: {error}");
                    log::error!("Recovery save failed: {error}");
                } else {
                    self.checkpointed.insert(state.doc.id.0);
                }
            }
        }
        self.last_recovery = Instant::now();
    }
}

impl eframe::App for AndroidEditor {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        for error in self.recovery_errors.try_iter() {
            self.app.ui.status_error = true;
            self.app.ui.status = error.clone();
            log::error!("{error}");
        }
        let focused = ctx.input(|i| i.focused);
        if (self.focused && !focused) || self.last_recovery.elapsed() >= Duration::from_secs(5) {
            self.checkpoint();
        }
        self.focused = focused;
        ctx.request_repaint_after(if ctx.egui_wants_keyboard_input() { Duration::from_millis(100) } else { Duration::from_secs(5) });
    }
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        // winit 0.30 does not forward NativeActivity ContentRectChanged. Keep
        // text fields and dialogs above the IME using Android's safe content area.
        let r = self.android_app.content_rect();
        // DeX reports the public desk configuration, even for maximized windows.
        let desktop = self.android_app.config().ui_mode_type() == android_activity::ndk::configuration::UiModeType::Desk;
        if self.fullscreen.update(input.focused, [r.left, r.top, r.right, r.bottom], desktop) {
            crate::fullscreen::apply(&self.android_app, desktop);
        }
        if self.content_surface.load(std::sync::atomic::Ordering::Acquire) {
            crate::viewport::surface_input(input, [r.left, r.top, r.right, r.bottom], ctx.pixels_per_point());
        } else if let Some(rect) = crate::viewport::content_rect([r.left, r.top, r.right, r.bottom], ctx.pixels_per_point()) {
            input.screen_rect = Some(rect);
        }
        self.app.android_touch_input(ctx, input);
        self.app.raw_input_hook(ctx, input);
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
        self.picker.show(ui.ctx());
    }
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.checkpoint();
        if let Ok(mut recovery) = self.recovery.try_borrow_mut() {
            for error in recovery.flush() {
                self.app.ui.status_error = true;
                self.app.ui.status = format!("Recovery save failed: {error}");
                log::error!("Recovery save failed: {error}");
            }
        }
    }
    fn persist_egui_memory(&self) -> bool {
        false
    }
    fn on_exit(&mut self) {
        self.checkpoint();
        if let Ok(mut recovery) = self.recovery.try_borrow_mut() {
            for error in recovery.flush() {
                log::error!("Recovery save failed: {error}");
            }
        }
    }
}
