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
    OpenCost,
    OpenSettings,
    Quit,
}

pub struct CodexBarTray {
    icon: IconPixmap,
    tooltip: String,
    provider_lines: Vec<String>,
    tx: Sender<TrayCommand>,
}

impl CodexBarTray {
    pub fn new(icon: IconPixmap, tooltip: String, tx: Sender<TrayCommand>) -> Self {
        CodexBarTray { icon, tooltip, provider_lines: Vec::new(), tx }
    }

    pub fn set_icon(&mut self, icon: IconPixmap) {
        self.icon = icon;
    }

    pub fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }

    pub fn set_provider_lines(&mut self, provider_lines: Vec<String>) {
        self.provider_lines = provider_lines;
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
        let mut items: Vec<MenuItem<Self>> = Vec::new();

        if self.provider_lines.is_empty() {
            items.push(StandardItem {
                label: "No provider usage loaded".into(),
                enabled: false,
                ..Default::default()
            }.into());
        } else {
            for line in self.provider_lines.iter().take(12) {
                items.push(StandardItem {
                    label: line.clone(),
                    enabled: false,
                    ..Default::default()
                }.into());
            }
        }

        items.push(MenuItem::Separator);
        items.push(StandardItem {
            label: "Show usage".into(),
            activate: Box::new(|t: &mut Self| {
                let _ = t.tx.send_blocking(TrayCommand::ToggleWindow);
            }),
            ..Default::default()
        }.into());
        items.push(StandardItem {
            label: "Refresh now".into(),
            activate: Box::new(|t: &mut Self| {
                let _ = t.tx.send_blocking(TrayCommand::RefreshNow);
            }),
            ..Default::default()
        }.into());
        items.push(StandardItem {
            label: "Open panel utility".into(),
            activate: Box::new(|t: &mut Self| {
                let _ = t.tx.send_blocking(TrayCommand::OpenPanelUtility);
            }),
            ..Default::default()
        }.into());
        items.push(StandardItem {
            label: "Cost & tokens…".into(),
            activate: Box::new(|t: &mut Self| {
                let _ = t.tx.send_blocking(TrayCommand::OpenCost);
            }),
            ..Default::default()
        }.into());
        items.push(StandardItem {
            label: "Settings…".into(),
            activate: Box::new(|t: &mut Self| {
                let _ = t.tx.send_blocking(TrayCommand::OpenSettings);
            }),
            ..Default::default()
        }.into());
        items.push(MenuItem::Separator);
        items.push(StandardItem {
            label: "Quit".into(),
            activate: Box::new(|t: &mut Self| {
                let _ = t.tx.send_blocking(TrayCommand::Quit);
            }),
            ..Default::default()
        }.into());
        items
    }
}
