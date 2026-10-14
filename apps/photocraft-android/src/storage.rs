//! Android document locations and validation, independent of Android runtime APIs.
use std::path::{Component, Path, PathBuf};

pub fn document_directory(external_files: &Path) -> Result<PathBuf, String> {
    let package = external_files.parent().ok_or("Missing Android package directory")?;
    let data = package.parent().ok_or("Missing Android data directory")?;
    if external_files.file_name().is_none_or(|n| n != "files") || data.file_name().is_none_or(|n| n != "data") {
        return Err("Unexpected Android external files location".into());
    }
    let android = data.parent().ok_or("Missing Android storage directory")?;
    let name = package.file_name().ok_or("Missing Android package name")?;
    Ok(android.join("media").join(name).join("Documents"))
}

pub fn save_path(directory: &Path, name: &str) -> Result<PathBuf, String> {
    let mut parts = Path::new(name).components();
    if name.trim().is_empty()
        || name.contains(['\0', '/', '\\'])
        || name.len() > 240
        || !matches!(parts.next(), Some(Component::Normal(_)))
        || parts.next().is_some()
    {
        return Err("Enter a file name without folders (for example Painting.pcraft)".into());
    }
    Ok(directory.join(name))
}

/// Bound even private settings files: corrupt or manually replaced files are untrusted.
pub fn read_text(path: &Path, limit: u64) -> Result<String, String> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path).map_err(|e| e.to_string())?.take(limit.saturating_add(1)).read_to_string(&mut text).map_err(|e| e.to_string())?;
    if text.len() as u64 > limit {
        return Err("Settings file exceeds its size limit".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_are_in_the_apps_shared_media_directory() {
        assert_eq!(
            document_directory(Path::new("/storage/emulated/0/Android/data/ai.storyteller.photocraft/files")).unwrap(),
            PathBuf::from("/storage/emulated/0/Android/media/ai.storyteller.photocraft/Documents")
        );
        assert!(document_directory(Path::new("/unexpected/files")).is_err());
    }

    #[test]
    fn save_name_cannot_escape_directory() {
        let dir = Path::new("/documents");
        for name in ["", ".", "..", "../lost.psd", "/lost.psd", "sub/file.psd", "bad\0.psd", "  "] {
            assert!(save_path(dir, name).is_err(), "{name:?}");
        }
        assert_eq!(save_path(dir, "Painting.pcraft").unwrap(), dir.join("Painting.pcraft"));
        assert_eq!(save_path(dir, "絵.psd").unwrap(), dir.join("絵.psd"));
    }
    #[test]
    fn corrupt_settings_are_bounded() {
        let path = std::env::temp_dir().join(format!("photocraft-android-settings-{}", std::process::id()));
        std::fs::write(&path, b"12345").unwrap();
        assert!(read_text(&path, 4).is_err());
        assert_eq!(read_text(&path, 5).unwrap(), "12345");
        std::fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(read_text(&path, 5).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
