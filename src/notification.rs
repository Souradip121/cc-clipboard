use std::path::Path;

pub fn notify_saved(path: &Path) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("screenshot");
    let msg = format!("Saved — path copied: {}", name);
    let _ = win_toast_notify::WinToastNotify::new()
        .set_title("cc-clipboard")
        .set_messages(vec![msg.as_str()])
        .show();
}
