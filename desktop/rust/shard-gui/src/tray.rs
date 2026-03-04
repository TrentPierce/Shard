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

        let icon = crate::make_tray_icon();

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

}
