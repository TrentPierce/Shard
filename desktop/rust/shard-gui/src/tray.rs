use anyhow::Result;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

pub struct TrayManager {
    _tray_icon: Option<TrayIcon>,
    pub _menu_channel: tray_icon::menu::MenuEventReceiver,
}

impl TrayManager {
    pub fn new() -> Result<Self> {
        let tray_menu = Menu::new();

        let show_item = MenuItem::new("Open Dashboard", true, None);
        let pause_item = MenuItem::new("Pause Node", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        let _show_id = show_item.id().clone();
        let _pause_id = pause_item.id().clone();
        tray_menu.append_items(&[
            &show_item,
            &PredefinedMenuItem::separator(),
            &pause_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])?;

        let icon = Self::load_icon();

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Shard Node")
            .with_icon(icon)
            .build()?;

        // Optionally, register IDs for handling
        Ok(Self {
            _tray_icon: Some(tray_icon),
            _menu_channel: MenuEvent::receiver().clone(),
        })
    }

    fn load_icon() -> tray_icon::Icon {
        // For now, return a blank/default icon or load a bytes resource
        // In a real app, we'd load a .ico or .png
        let (icon_rgba, icon_width, icon_height) = (vec![255_u8; 32 * 32 * 4], 32, 32);
        tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height)
            .expect("Failed to create icon")
    }
}
