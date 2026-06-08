use crate::config::Config;
use image::{DynamicImage, RgbaImage};
use std::path::PathBuf;

pub fn capture_primary() -> Option<(RgbaImage, u32, u32)> {
    let monitors = xcap::Monitor::all().ok()?;
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary())
        .or_else(|| monitors.first())?;
    let w = monitor.width();
    let h = monitor.height();
    let img = monitor.capture_image().ok()?;
    Some((img, w, h))
}

pub fn crop_and_save(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32, config: &Config) -> Option<PathBuf> {
    if w == 0 || h == 0 {
        return None;
    }
    let cropped = DynamicImage::ImageRgba8(img.clone()).crop_imm(x, y, w, h);
    let filename = format!("cc_{}.png", chrono::Local::now().format("%Y%m%d_%H%M%S"));
    let path = config.save_folder.join(&filename);
    cropped.save(&path).ok()?;
    Some(path)
}
