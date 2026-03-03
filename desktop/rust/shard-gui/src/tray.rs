use anyhow::Result;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, MenuId, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

pub struct TrayManager {
    tray_icon: Option<TrayIcon>,
    pub show_id: MenuId,
    pub quit_id: MenuId,
    pub pause_id: MenuId,
    pub _menu_channel: tray_icon::menu::MenuEventReceiver,
}

impl TrayManager {
    pub fn new() -> Result<Self> {
        let tray_menu = Menu::new();

        let show_item = MenuItem::new("Open Dashboard", true, None);
        let pause_item = MenuItem::new("Pause Node", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        let show_id = show_item.id().clone();
        let pause_id = pause_item.id().clone();
        let quit_id = quit_item.id().clone();

        tray_menu.append_items(&[
            &show_item,
            &PredefinedMenuItem::separator(),
            &pause_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])?;

        let icon = Self::make_icon();

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Shard Node")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            tray_icon: Some(tray_icon),
            show_id,
            quit_id,
            pause_id,
            _menu_channel: MenuEvent::receiver().clone(),
        })
    }

    pub fn update_tooltip(&self, status: &str) {
        if let Some(tray) = &self.tray_icon {
            let _ = tray.set_tooltip(Some(status));
        }
    }

    fn make_icon() -> tray_icon::Icon {
        // Generate teal (#0EA5E9) filled circle in RGBA at 32x32
        let width: u32 = 32;
        let height: u32 = 32;
        let cx = width as f32 / 2.0;
        let cy = height as f32 / 2.0;
        let radius = (width as f32 / 2.0) - 1.0;

        let mut rgba = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - cx + 0.5;
                let dy = y as f32 - cy + 0.5;
                let dist = (dx * dx + dy * dy).sqrt();
                let idx = ((y * width + x) * 4) as usize;
                if dist <= radius {
                    rgba[idx] = 0x0E; // R
                    rgba[idx + 1] = 0xA5; // G
                    rgba[idx + 2] = 0xE9; // B
                    rgba[idx + 3] = 255; // A
                }
                // else remains 0,0,0,0 (transparent)
            }
        }

        tray_icon::Icon::from_rgba(rgba, width, height).expect("Failed to create icon")
    }
}
