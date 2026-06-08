use image::RgbaImage;
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::window::Window;

pub struct OverlayState {
    pub window: Arc<Window>,
    pub screenshot: RgbaImage,
    pub drag_start: Option<(u32, u32)>,
    pub drag_current: Option<(u32, u32)>,
    width: u32,
    height: u32,
    // Context must be declared before surface so it outlives it on drop
    // (Rust drops fields in declaration order, first declared = last dropped)
    _context: Context<Arc<Window>>,
    surface: Surface<Arc<Window>, Arc<Window>>,
}

impl OverlayState {
    pub fn new(window: Arc<Window>, screenshot: RgbaImage, width: u32, height: u32) -> Self {
        let context = Context::new(Arc::clone(&window)).expect("softbuffer context");
        let mut surface = Surface::new(&context, Arc::clone(&window)).expect("softbuffer surface");
        if let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
            let _ = surface.resize(w, h);
        }
        OverlayState {
            window,
            screenshot,
            drag_start: None,
            drag_current: None,
            width,
            height,
            _context: context,
            surface,
        }
    }

    pub fn draw(&mut self) {
        let w = self.width;
        let h = self.height;
        let Ok(mut buf) = self.surface.buffer_mut() else {
            return;
        };
        if buf.len() != (w * h) as usize {
            return;
        }
        let sel = match (self.drag_start, self.drag_current) {
            (Some(s), Some(c)) => Some((s.0, s.1, c.0, c.1)),
            _ => None,
        };
        fill_buffer(&mut buf, &self.screenshot, w, h, sel);
        let _ = buf.present();
    }
}

fn fill_buffer(
    buf: &mut [u32],
    img: &RgbaImage,
    w: u32,
    h: u32,
    sel: Option<(u32, u32, u32, u32)>,
) {
    let raw = img.as_raw();

    // Dim entire screenshot into buffer (55% brightness)
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let p = i * 4;
            if p + 2 < raw.len() {
                let r = (raw[p] as u32 * 55) / 100;
                let g = (raw[p + 1] as u32 * 55) / 100;
                let b = (raw[p + 2] as u32 * 55) / 100;
                buf[i] = (r << 16) | (g << 8) | b;
            }
        }
    }

    let Some((x1, y1, x2, y2)) = sel else {
        return;
    };

    let lx = x1.min(x2).min(w.saturating_sub(1));
    let rx = x1.max(x2).min(w.saturating_sub(1));
    let ty = y1.min(y2).min(h.saturating_sub(1));
    let by = y1.max(y2).min(h.saturating_sub(1));

    // Restore full brightness in selected region
    for y in ty..=by {
        for x in lx..=rx {
            let i = (y * w + x) as usize;
            let p = i * 4;
            if p + 2 < raw.len() {
                buf[i] = ((raw[p] as u32) << 16)
                    | ((raw[p + 1] as u32) << 8)
                    | (raw[p + 2] as u32);
            }
        }
    }

    // White 1px border around selection
    for x in lx..=rx {
        buf[(ty * w + x) as usize] = 0x00FF_FFFF;
        buf[(by * w + x) as usize] = 0x00FF_FFFF;
    }
    for y in ty..=by {
        buf[(y * w + lx) as usize] = 0x00FF_FFFF;
        buf[(y * w + rx) as usize] = 0x00FF_FFFF;
    }
}
