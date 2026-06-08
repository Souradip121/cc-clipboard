use std::path::Path;

pub fn copy_path(path: &Path) {
    if let Some(s) = path.to_str() {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(s);
        }
    }
}
