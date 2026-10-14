//! Android storage, codecs and crash recovery through the shared service seam.
use crate::crash_guard::guard;
use crate::recovery::Recovery;
use photocraft_doc::Document;
use photocraft_ui_egui::{ExportSettings, Recoverable, Services};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, mpsc::Sender},
};

fn options(settings: &ExportSettings) -> photocraft_io::ExportOptions {
    let mut opts = photocraft_io::ExportOptions::default();
    if let Some(q) = settings.jpeg_quality {
        opts.encode.jpeg_quality = q;
    }
    opts.encode.webp_lossless = settings.webp_lossless;
    if let Some(q) = settings.webp_quality {
        opts.encode.webp_quality = q;
    }
    opts.tiff_layers = settings.tiff_layers;
    opts.xmp = if settings.xmp_all { photocraft_io::XmpEmbed::All } else { photocraft_io::XmpEmbed::None };
    opts.tga_bits = settings.tga_bits;
    opts.image_previews = settings.image_previews;
    opts.psd_maximize_compatibility = settings.psd_maximize_compatibility;
    opts
}

fn export(doc: &Document, path: &str, settings: &ExportSettings) -> Result<(Vec<u8>, Vec<String>), String> {
    guard("Save", || photocraft_io::export(doc, path, &options(settings)).map(|r| (r.bytes, r.warnings)).map_err(|e| e.to_string()))
}

pub type NativeServices = (Services, Rc<RefCell<Recovery>>, std::sync::mpsc::Receiver<String>);

pub fn native(private: PathBuf, requests: Sender<crate::picker::Request>, workspace: Option<photocraft_automation::AuthorizedWorkspace>) -> NativeServices {
    let (recovery_errors, errors) = std::sync::mpsc::channel();
    let prefs = private.join("preferences.json");
    let prefs_save = prefs.clone();
    let recovery_dir = private.join("recovery");
    let store = Rc::new(RefCell::new(Recovery::new(recovery_dir.clone())));
    let (s1, s2, s3, s4) = (store.clone(), store.clone(), store.clone(), store.clone());
    let automation_read = workspace.clone().map(|w| {
        Box::new(move |path: &str| {
            let bytes = w.read(path).map_err(|e| e.to_string())?;
            Ok((path.to_string(), bytes))
        }) as photocraft_ui_egui::AutomationReadFn
    });
    let automation_write = workspace
        .clone()
        .map(|w| Box::new(move |path: &str, bytes: &[u8]| w.write(path, bytes).map_err(|e| e.to_string())) as photocraft_ui_egui::AutomationWriteFn);
    let automation_open_pcraft = workspace.clone().map(|w| {
        Box::new(move |path: &str| guard("Open", || w.open_pcraft(path).map(|d| d.map(|doc| (path.to_string(), doc))).map_err(|e| e.to_string())))
            as photocraft_ui_egui::AutomationOpenPcraftFn
    });
    let automation_save = workspace.clone().map(|w| {
        Arc::new(move |doc: &Document, path: &str, settings: &ExportSettings, ctl: &photocraft_ui_egui::jobs_ui::SaveCtl| {
            ctl.check()?;
            let (bytes, warnings) = export(doc, path, settings)?;
            ctl.commit()?;
            w.write(path, &bytes).map_err(|e| e.to_string())?;
            Ok(warnings)
        }) as photocraft_ui_egui::SaveFileFn
    });
    let services = Services {
        import: Some(Box::new(|name, bytes, depth| {
            guard("Open", || photocraft_io::import_with_svg_group_depth(name, bytes, depth).map(|r| (r.document, r.warnings)).map_err(|e| e.to_string()))
        })),
        export: Some(Box::new(export)),
        write: Some(Box::new(|path, bytes| photocraft_format::atomic::atomic_write(Path::new(path), bytes).map_err(|e| e.to_string()))),
        save_file: Some(Arc::new(|doc, path, settings, ctl| {
            ctl.check()?;
            let (bytes, warnings) = export(doc, path, settings)?;
            ctl.commit()?;
            photocraft_format::atomic::atomic_write(Path::new(path), &bytes).map_err(|e| e.to_string())?;
            Ok(warnings)
        })),
        file_dialog: Some(Box::new(move |request, _, reply| {
            if let Err(e) = requests.send((request, reply)) {
                e.0.1.send(Some(photocraft_ui_egui::FileDialogAnswer::Unavailable("The document browser is unavailable".into())));
            }
        })),
        encode_png: Some(Box::new(|w, h, rgba| {
            let image = photocraft_codecs::Image::from_u8(w, h, photocraft_codecs::ChannelLayout::Rgba, rgba.to_vec()).map_err(|e| e.to_string())?;
            photocraft_codecs::encode(&image, photocraft_codecs::Format::Png, &Default::default()).map_err(|e| e.to_string())
        })),
        load_prefs: Some(Box::new(move || crate::storage::read_text(&prefs, 4 * 1024 * 1024).ok())),
        save_prefs: Some(Box::new(move |text| photocraft_format::atomic::atomic_write(&prefs_save, text.as_bytes()).map_err(|e| e.to_string()))),
        autosave: Some(Box::new(move |doc, revision, path| s1.try_borrow_mut().map_err(|e| e.to_string())?.autosave(doc, revision, path.map(str::to_owned)))),
        autosave_results: Some(Box::new(move || {
            let results = s2.try_borrow_mut().map(|mut s| s.take_completed()).unwrap_or_default();
            for (_, _, result) in &results {
                if let Err(error) = result {
                    let _ = recovery_errors.send(format!("Recovery save failed: {error}"));
                }
            }
            results
        })),
        discard_autosave: Some(Box::new(move |id| {
            if let Ok(mut s) = s3.try_borrow_mut()
                && let Err(e) = s.discard(id)
            {
                log::warn!("Could not discard recovery: {e}");
            }
        })),
        adopt_autosave: Some(Box::new(move |id, key| {
            if let Ok(mut s) = s4.try_borrow_mut() {
                s.adopt(id, key);
            }
        })),
        recover: Some(Box::new(move || {
            photocraft_format::list_recovery(&recovery_dir)
                .into_iter()
                .map(|entry| Recoverable {
                    key: entry.info.key.clone(),
                    name: entry.info.document_name.clone(),
                    path: entry.info.original_path.clone(),
                    load: Box::new(move || guard("Recover", || photocraft_format::recover(&entry).map_err(|e| e.to_string()))),
                })
                .collect()
        })),
        automation_read,
        automation_write,
        automation_open_pcraft,
        automation_save,
        automation_authorize: workspace
            .as_ref()
            .map(|_| photocraft_automation::workspace::authorize_desktop_engine_step as fn(&str, &serde_json::Value) -> photocraft_engine::Result<()>),
        automation_command: workspace.map(|_| {
            Box::new(|id: &str, params: &serde_json::Value| {
                photocraft_automation::workspace::authorize_desktop_engine_command(id, params).map_err(|e| e.to_string())
            }) as photocraft_ui_egui::AutomationCommandFn
        }),
        ..Default::default()
    };
    (services, store, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_settings_preserve_user_choices() {
        let settings = ExportSettings { jpeg_quality: Some(73), webp_quality: Some(81), webp_lossless: false, ..Default::default() };
        let opts = options(&settings);
        assert_eq!(opts.encode.jpeg_quality, 73);
        assert_eq!(opts.encode.webp_quality, 81);
        assert!(!opts.encode.webp_lossless);
    }
    #[test]
    fn android_services_round_trip_documents_preferences_and_errors() {
        let root = std::env::temp_dir().join(format!("photocraft-android-services-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let (_picker, tx) = crate::picker::Picker::new(root.clone());
        let (mut services, _recovery, _errors) = native(root.clone(), tx, None);
        (services.save_prefs.as_mut().unwrap())("{\"androidTest\":true}").unwrap();
        assert_eq!((services.load_prefs.as_mut().unwrap())().as_deref(), Some("{\"androidTest\":true}"));
        for depth in [8, 16, 32] {
            let mut session = photocraft_engine::Session::new();
            session.execute("file.new", serde_json::json!({"name":"Android test", "width":8,"height":8,"depth":depth,"background":"white"})).unwrap();
            let doc = &session.active().unwrap().doc;
            let path = root.join(format!("roundtrip-{depth}.pcraft"));
            let path = path.to_str().unwrap();
            let (bytes, _) = (services.export.as_ref().unwrap())(doc, path, &ExportSettings::default()).unwrap();
            (services.write.as_mut().unwrap())(path, &bytes).unwrap();
            let (reopened, _) = (services.import.as_ref().unwrap())(path, &std::fs::read(path).unwrap(), 32).unwrap();
            assert_eq!(reopened.depth, doc.depth);
            assert_eq!(reopened.size, doc.size);
            assert_eq!(reopened.layers.len(), doc.layers.len());
        }
        assert!((services.import.as_ref().unwrap())("corrupt.psd", b"not a PSD", 32).is_err());
        std::fs::write(root.join("blocked"), b"file, not directory").unwrap();
        assert!((services.write.as_mut().unwrap())(root.join("blocked/out.psd").to_str().unwrap(), b"data").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
