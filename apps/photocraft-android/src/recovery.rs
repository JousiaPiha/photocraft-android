//! Android recovery with an explicit completion barrier for activity suspension.
use photocraft_doc::Document;
use photocraft_format::Autosaver;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

pub type Completion = (u64, u64, Result<(), String>);

/// Owns the same format-crate workers used by desktop recovery. Android additionally
/// needs to finish outstanding writes before its suspended activity can be killed.
pub struct Recovery {
    directory: PathBuf,
    prefix: String,
    keys: HashMap<u64, String>,
    savers: HashMap<u64, Autosaver>,
    revisions: HashMap<u64, u64>,
    completed: Vec<Completion>,
}

impl Recovery {
    pub fn new(directory: PathBuf) -> Self {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        Self {
            directory,
            prefix: format!("android-{}-{now}", std::process::id()),
            keys: HashMap::new(),
            savers: HashMap::new(),
            revisions: HashMap::new(),
            completed: Vec::new(),
        }
    }
    pub fn adopt(&mut self, id: u64, key: &str) {
        self.keys.insert(id, key.to_owned());
    }
    pub fn autosave(&mut self, doc: &Arc<Document>, revision: u64, path: Option<String>) -> Result<(), String> {
        let id = doc.id.0;
        if self.revisions.get(&id) == Some(&revision) {
            return Ok(());
        }
        let key = self.keys.entry(id).or_insert_with(|| format!("{}-{id}", self.prefix));
        let saver = self.savers.entry(id).or_insert_with(|| Autosaver::new(&self.directory, key));
        saver.request_checked(doc.clone(), revision, path, Default::default())?;
        self.revisions.insert(id, revision);
        Ok(())
    }
    pub fn take_completed(&mut self) -> Vec<Completion> {
        let mut results = std::mem::take(&mut self.completed);
        for (&id, saver) in &self.savers {
            results.extend(saver.take_completed().into_iter().map(|(revision, result)| (id, revision, result)));
        }
        for (id, revision, result) in &results {
            if result.is_err() && self.revisions.get(id) == Some(revision) {
                self.revisions.remove(id);
            }
        }
        results
    }
    /// Join pending writers. Called by eframe's Android suspension `App::save` hook.
    pub fn flush(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        for (id, saver) in std::mem::take(&mut self.savers) {
            if let Some(result) = saver.flush() {
                let result = result.map(|_| ()).map_err(|e| e.to_string());
                if let Err(error) = &result {
                    errors.push(error.clone());
                }
                let revision = self.revisions.get(&id).copied().unwrap_or(0);
                self.completed.push((id, revision, result));
            }
        }
        errors
    }
    pub fn discard(&mut self, id: u64) -> Result<(), String> {
        self.revisions.remove(&id);
        if let Some(saver) = self.savers.remove(&id) {
            saver.discard().map_err(|e| e.to_string())?;
        }
        if let Some(key) = self.keys.remove(&id) {
            for entry in photocraft_format::list_recovery(&self.directory).into_iter().filter(|e| e.info.key == key) {
                photocraft_format::discard_recovery(&self.directory, &entry).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> Arc<Document> {
        let mut session = photocraft_engine::Session::new();
        session.execute("file.new", serde_json::json!({"width":8,"height":8})).unwrap();
        session.active().unwrap().doc.clone()
    }
    #[test]
    fn suspension_waits_for_snapshot_and_saved_documents_are_discarded() {
        let root = std::env::temp_dir().join(format!("photocraft-android-recovery-{}", std::process::id()));
        let mut recovery = Recovery::new(root.clone());
        let doc = document();
        recovery.autosave(&doc, 2, None).unwrap();
        assert!(recovery.flush().is_empty());
        let entries = photocraft_format::list_recovery(&root);
        assert_eq!(entries.len(), 1);
        assert_eq!(photocraft_format::recover(&entries[0]).unwrap().size, doc.size);
        recovery.autosave(&doc, 2, None).unwrap();
        assert!(recovery.savers.is_empty(), "unchanged snapshot must not start another writer");
        recovery.autosave(&doc, 3, None).unwrap();
        assert!(recovery.flush().is_empty());
        assert_eq!(photocraft_format::list_recovery(&root).len(), 1, "resume reuses the recovery key");
        recovery.discard(doc.id.0).unwrap();
        assert!(photocraft_format::list_recovery(&root).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn background_write_errors_are_reported_and_retryable() {
        let root = std::env::temp_dir().join(format!("photocraft-android-blocked-{}", std::process::id()));
        std::fs::write(&root, b"not a directory").unwrap();
        let doc = document();
        let mut recovery = Recovery::new(root.clone());
        recovery.autosave(&doc, 2, None).unwrap();
        assert!(!recovery.flush().is_empty());
        assert!(recovery.take_completed().iter().any(|(_, _, r)| r.is_err()));
        assert!(!recovery.revisions.contains_key(&doc.id.0));
        std::fs::remove_file(root).unwrap();
    }
}
