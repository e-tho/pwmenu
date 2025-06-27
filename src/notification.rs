use anyhow::{anyhow, Result};
use notify_rust::{Notification, NotificationHandle, Timeout};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{icons::Icons, pw::NodeType};

pub struct NotificationManager {
    icons: Arc<Icons>,
    handles: Arc<Mutex<HashMap<u32, NotificationHandle>>>,
}

impl NotificationManager {
    pub fn new(icons: Arc<Icons>) -> Self {
        Self {
            icons,
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_icons_default() -> Self {
        Self::new(Arc::new(Icons::default()))
    }

    pub fn send_notification(
        &self,
        summary: Option<String>,
        body: Option<String>,
        icon: Option<&str>,
        timeout: Option<Timeout>,
    ) -> Result<u32> {
        let icon_name = self.icons.get_xdg_icon(icon.unwrap_or("output"));

        let mut notification = Notification::new();
        notification
            .summary(summary.as_deref().unwrap_or("PipeWire Menu"))
            .body(body.as_deref().unwrap_or(""))
            .icon(&icon_name)
            .timeout(timeout.unwrap_or(Timeout::Milliseconds(3000)));

        let handle = notification.show()?;
        let id = handle.id();

        let mut handles = self
            .handles
            .lock()
            .map_err(|e| anyhow!("Failed to acquire lock on notification handles: {}", e))?;
        handles.insert(id, handle);

        Ok(id)
    }

    pub fn close_notification(&self, id: u32) -> Result<()> {
        let mut handles = self
            .handles
            .lock()
            .map_err(|e| anyhow!("Failed to acquire lock on notification handles: {}", e))?;

        if let Some(handle) = handles.remove(&id) {
            handle.close();
            Ok(())
        } else {
            Err(anyhow!("Notification ID {} not found", id))
        }
    }

    fn get_volume_notification_icon_key(
        &self,
        node_type: &NodeType,
        volume_percent: u8,
        is_muted: bool,
    ) -> &str {
        if is_muted {
            match node_type {
                NodeType::Sink => "output_mute",
                NodeType::Source => "input_mute",
                _ => "output_mute",
            }
        } else {
            let volume_level = if volume_percent > 70 {
                "high"
            } else if volume_percent > 30 {
                "medium"
            } else {
                "low"
            };

            match (node_type, volume_level) {
                (NodeType::Sink, "high") => "output_volume_high",
                (NodeType::Sink, "medium") => "output_volume_medium",
                (NodeType::Sink, "low") => "output_volume_low",
                (NodeType::Source, "high") => "input_volume_high",
                (NodeType::Source, "medium") => "input_volume_medium",
                (NodeType::Source, "low") => "input_volume_low",
                _ => "output_volume_medium",
            }
        }
    }

    pub fn send_volume_notification(
        &self,
        device_name: &str,
        volume_percent: u8,
        is_muted: bool,
        node_type: &NodeType,
    ) -> Result<u32> {
        let icon_key = self.get_volume_notification_icon_key(node_type, volume_percent, is_muted);

        let summary = format!("Volume: {volume_percent}%");
        let body = if is_muted {
            format!("{device_name} is muted")
        } else {
            device_name.to_string()
        };

        self.send_notification(Some(summary), Some(body), Some(icon_key), None)
    }

    pub fn send_default_changed_notification(
        &self,
        device_type: &str,
        device_name: &str,
    ) -> Result<u32> {
        let icon = if device_type == "output" {
            "output"
        } else {
            "input"
        };
        let summary = format!("Default {device_type} changed");
        let body = format!("{device_name} is now the default {device_type}");

        self.send_notification(Some(summary), Some(body), Some(icon), None)
    }
}
