//! StatusNotifierItem tray via ksni. Runs on its own thread; user actions are
//! delivered to the GTK main loop through a channel of `TrayCommand`.

use crate::icon_renderer::IconPixmap;
use ksni::{Icon, MenuItem, ToolTip, Tray};
use ksni::menu::StandardItem;
use async_channel::Sender;

/// Commands the tray emits toward the GTK main loop.
#[derive(Debug, Clone)]
pub enum TrayCommand {
    ToggleWindow,
    RefreshNow,
    OpenPanelUtility,
    OpenSettings,
    Quit,
}

pub struct CodexBarTray {
    icon: IconPixmap,
    tooltip: String,
    tx: Sender<TrayCommand>,
}

impl CodexBarTray {
    pub fn new(icon: IconPixmap, tooltip: String, tx: Sender<TrayCommand>) -> Self {
        CodexBarTray { icon, tooltip, tx }
    }

    pub fn set_icon(&mut self, icon: IconPixmap) {
        self.icon = icon;
    }

    pub fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }
}

impl Tray for CodexBarTray {
    fn id(&self) -> String {
        "codexbar-tray".into()
    }

    fn title(&self) -> String {
        "CodexBar".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![Icon {
            width: self.icon.width,
            height: self.icon.height,
            data: self.icon.data.clone(),
        }]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "CodexBar".into(),
            description: self.tooltip.clone(),
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send_blocking(TrayCommand::ToggleWindow);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Show usage".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send_blocking(TrayCommand::ToggleWindow);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Refresh now".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send_blocking(TrayCommand::RefreshNow);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open panel utility".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send_blocking(TrayCommand::OpenPanelUtility);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Settings…".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send_blocking(TrayCommand::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send_blocking(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
