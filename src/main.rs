#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod clipboard;
mod config;
mod hotkey;
mod notification;
mod overlay;

use config::Config;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use overlay::OverlayState;
use std::sync::Arc;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Cursor, CursorIcon, Fullscreen, WindowAttributes, WindowId},
};

// ── tray icon: 16×16 solid blue-green square ──────────────────────────────────

fn make_icon() -> tray_icon::Icon {
    const S: usize = 16;
    let mut rgba = vec![0u8; S * S * 4];
    for i in 0..S * S {
        rgba[i * 4] = 0x00; // R
        rgba[i * 4 + 1] = 0xBB; // G
        rgba[i * 4 + 2] = 0xFF; // B
        rgba[i * 4 + 3] = 0xFF; // A
    }
    tray_icon::Icon::from_rgba(rgba, S as u32, S as u32).expect("icon")
}

// ── App state ─────────────────────────────────────────────────────────────────

struct MenuIds {
    screenshot: tray_icon::menu::MenuId,
    open_folder: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

struct App {
    config: Config,
    // Kept alive so the tray icon stays visible
    _tray: Option<TrayIcon>,
    menu_ids: Option<MenuIds>,
    // Kept alive to maintain hotkey registration
    _hotkey_manager: Option<GlobalHotKeyManager>,
    hotkey_id: u32,
    overlay: Option<OverlayState>,
}

impl App {
    fn new() -> Self {
        App {
            config: Config::load(),
            _tray: None,
            menu_ids: None,
            _hotkey_manager: None,
            hotkey_id: 0,
            overlay: None,
        }
    }

    fn start_capture(&mut self, event_loop: &ActiveEventLoop) {
        if self.overlay.is_some() {
            return;
        }
        let Some((screenshot, mon_w, mon_h)) = capture::capture_primary() else {
            return;
        };

        let attrs = WindowAttributes::default()
            .with_title("cc-clipboard-overlay")
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
            .with_fullscreen(Some(Fullscreen::Borderless(None)));

        let Ok(window) = event_loop.create_window(attrs) else {
            return;
        };
        window.set_cursor(Cursor::Icon(CursorIcon::Crosshair));
        let window = Arc::new(window);

        // Use the actual window inner_size in case it differs from xcap dimensions
        let inner = window.inner_size();
        let (w, h) = if inner.width > 0 && inner.height > 0 {
            (inner.width, inner.height)
        } else {
            (mon_w, mon_h)
        };

        self.overlay = Some(OverlayState::new(window, screenshot, w, h));
    }

    fn finish_capture(&mut self, img: image::RgbaImage, x1: u32, y1: u32, x2: u32, y2: u32) {
        let lx = x1.min(x2);
        let rx = x1.max(x2);
        let ty = y1.min(y2);
        let by = y1.max(y2);

        // Ignore accidental clicks (< 5px in either dimension)
        if rx.saturating_sub(lx) < 5 || by.saturating_sub(ty) < 5 {
            return;
        }

        if let Some(path) =
            capture::crop_and_save(&img, lx, ty, rx - lx, by - ty, &self.config)
        {
            clipboard::copy_path(&path);
            if self.config.notify {
                notification::notify_saved(&path);
            }
        }
    }

    fn open_folder(&self) {
        let _ = std::process::Command::new("explorer")
            .arg(&self.config.save_folder)
            .spawn();
    }
}

// ── ApplicationHandler ────────────────────────────────────────────────────────

impl ApplicationHandler for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // Build tray context menu
        let screenshot_item = MenuItem::new("Screenshot  (Ctrl+Shift+S)", true, None);
        let open_folder_item = MenuItem::new("Open Folder", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        self.menu_ids = Some(MenuIds {
            screenshot: screenshot_item.id().clone(),
            open_folder: open_folder_item.id().clone(),
            quit: quit_item.id().clone(),
        });

        let menu = Menu::new();
        let _ = menu.append_items(&[
            &screenshot_item,
            &open_folder_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ]);

        self._tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("cc-clipboard")
            .with_icon(make_icon())
            .build()
            .ok();

        // Register global hotkey
        if let Ok(manager) = GlobalHotKeyManager::new() {
            let hk = hotkey::parse_hotkey(&self.config.hotkey);
            self.hotkey_id = hk.id();
            let _ = manager.register(hk);
            self._hotkey_manager = Some(manager);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::RedrawRequested => {
                if let Some(ov) = &mut self.overlay {
                    ov.draw();
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if let Some(ov) = &mut self.overlay {
                    ov.drag_current = Some((position.x as u32, position.y as u32));
                    ov.window.request_redraw();
                }
            }

            WindowEvent::MouseInput { button, state, .. } => {
                match (button, state) {
                    (MouseButton::Left, ElementState::Pressed) => {
                        if let Some(ov) = &mut self.overlay {
                            ov.drag_start = ov.drag_current;
                        }
                    }
                    (MouseButton::Left, ElementState::Released) => {
                        // Extract everything from overlay before dropping it
                        let result = self.overlay.take().and_then(|ov| {
                            let s = ov.drag_start?;
                            let c = ov.drag_current?;
                            Some((ov.screenshot, s.0, s.1, c.0, c.1))
                        });
                        if let Some((img, x1, y1, x2, y2)) = result {
                            self.finish_capture(img, x1, y1, x2, y2);
                        }
                    }
                    // Right-click cancels
                    (MouseButton::Right, ElementState::Pressed) => {
                        self.overlay = None;
                    }
                    _ => {}
                }
            }

            WindowEvent::KeyboardInput { event: key, .. } => {
                use winit::keyboard::{KeyCode, PhysicalKey};
                if key.physical_key == PhysicalKey::Code(KeyCode::Escape) {
                    self.overlay = None;
                }
            }

            // Close overlay if it loses focus (e.g. Alt+Tab)
            WindowEvent::Focused(false) => {
                self.overlay = None;
            }

            WindowEvent::CloseRequested => {
                if self.overlay.is_some() {
                    self.overlay = None;
                } else {
                    event_loop.exit();
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Poll tray menu events
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if let Some(ids) = &self.menu_ids {
                if ev.id == ids.screenshot {
                    self.start_capture(event_loop);
                } else if ev.id == ids.open_folder {
                    self.open_folder();
                } else if ev.id == ids.quit {
                    event_loop.exit();
                }
            }
        }

        // Poll global hotkey events
        while let Ok(ev) = GlobalHotKeyEvent::receiver().try_recv() {
            if ev.id() == self.hotkey_id && ev.state() == HotKeyState::Pressed {
                self.start_capture(event_loop);
            }
        }

        // Use Poll when overlay is open (responsive redraws), Wait otherwise (low CPU)
        if self.overlay.is_some() {
            event_loop.set_control_flow(ControlFlow::Poll);
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run");
}
