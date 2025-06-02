use crate::pw::{graph::Store, nodes::NodeType};
use anyhow::{anyhow, Result};
use log::debug;
use std::{collections::HashMap, time::Instant};

const RESTORATION_TIMEOUT_SECS: u64 = 30;
const MAX_RESTORATION_ATTEMPTS: u8 = 50;

#[derive(Debug, Clone)]
pub struct DefaultRestoration {
    pub device_id: u32,
    pub device_name: String,
    pub had_default_sink: bool,
    pub had_default_source: bool,
    pub target_profile_index: u32,
    timestamp: Instant,
    attempts: u8,
}

impl DefaultRestoration {
    pub fn new(
        device_id: u32,
        device_name: String,
        had_default_sink: bool,
        had_default_source: bool,
        target_profile_index: u32,
    ) -> Self {
        Self {
            device_id,
            device_name,
            had_default_sink,
            had_default_source,
            target_profile_index,
            timestamp: Instant::now(),
            attempts: 0,
        }
    }

    fn is_expired(&self) -> bool {
        self.timestamp.elapsed().as_secs() > RESTORATION_TIMEOUT_SECS
    }

    fn max_attempts_reached(&self) -> bool {
        self.attempts >= MAX_RESTORATION_ATTEMPTS
    }

    fn increment_attempt(&mut self) {
        self.attempts += 1;
    }
}

#[derive(Debug, Default)]
pub struct RestorationManager {
    pending: HashMap<String, DefaultRestoration>,
}

impl RestorationManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn should_capture_defaults(store: &Store, device_id: u32) -> Option<(String, bool, bool)> {
        let device = store.devices.get(&device_id)?;

        // Only handle USB devices
        if !Self::is_usb_device(&device.name) {
            return None;
        }

        // Check current defaults for this device
        let had_default_sink = store.nodes.values().any(|n| {
            n.device_id == Some(device_id) && n.is_default && matches!(n.node_type, NodeType::Sink)
        });
        let had_default_source = store.nodes.values().any(|n| {
            n.device_id == Some(device_id)
                && n.is_default
                && matches!(n.node_type, NodeType::Source)
        });

        if !had_default_sink && !had_default_source {
            return None;
        }

        Some((device.name.clone(), had_default_sink, had_default_source))
    }

    pub fn capture_defaults(
        &mut self,
        device_id: u32,
        device_name: String,
        had_default_sink: bool,
        had_default_source: bool,
        target_profile_index: u32,
    ) {
        let restoration = DefaultRestoration::new(
            device_id,
            device_name.clone(),
            had_default_sink,
            had_default_source,
            target_profile_index,
        );

        debug!(
            "Capturing USB defaults for {}: sink={}, source={}",
            device_name, had_default_sink, had_default_source
        );

        self.pending.insert(device_name, restoration);
    }

    pub fn get_pending_restorations(&self, store: &Store) -> (Vec<(u32, u32)>, Vec<String>) {
        let mut nodes_to_restore = Vec::new();
        let mut completed_devices = Vec::new();

        for (device_name, restoration) in &self.pending {
            if restoration.is_expired() || restoration.max_attempts_reached() {
                continue;
            }

            match Self::attempt_restoration(store, restoration) {
                Ok(Some((sink_ids, source_ids))) => {
                    let sink_id = sink_ids.first().copied().unwrap_or(0);
                    let source_id = source_ids.first().copied().unwrap_or(0);
                    nodes_to_restore.push((sink_id, source_id));
                    completed_devices.push(device_name.clone());
                }
                Ok(None) => {}
                Err(_e) => {}
            }
        }

        (nodes_to_restore, completed_devices)
    }

    pub fn update_attempts_and_cleanup(&mut self) {
        let mut to_remove = Vec::new();

        for (device_name, restoration) in &mut self.pending {
            if restoration.is_expired() {
                debug!("Restoration expired for device {}", device_name);
                to_remove.push(device_name.clone());
            } else if restoration.max_attempts_reached() {
                debug!("Max attempts reached for device {}", device_name);
                to_remove.push(device_name.clone());
            } else {
                restoration.increment_attempt();
            }
        }

        for device_name in to_remove {
            self.pending.remove(&device_name);
        }
    }

    pub fn mark_completed(&mut self, device_names: &[String]) {
        for device_name in device_names {
            if self.pending.remove(device_name).is_some() {
                debug!("Successfully restored defaults for device {}", device_name);
            }
        }
    }

    fn attempt_restoration(
        store: &Store,
        restoration: &DefaultRestoration,
    ) -> Result<Option<(Vec<u32>, Vec<u32>)>> {
        // Find the device and check if profile change is complete
        let device = store
            .devices
            .values()
            .find(|d| d.name == restoration.device_name)
            .ok_or_else(|| anyhow!("Device {} not found", restoration.device_name))?;

        // Verify profile change is complete
        if device.current_profile_index != Some(restoration.target_profile_index) {
            return Ok(None);
        }

        // Check if expected nodes are available
        let has_nodes = store.nodes.values().any(|n| n.device_id == Some(device.id));
        if !has_nodes {
            return Ok(None);
        }

        let mut sink_ids = Vec::new();
        let mut source_ids = Vec::new();

        // Collect sink nodes to restore as default
        if restoration.had_default_sink {
            if let Some(sink_node) = store
                .nodes
                .values()
                .find(|n| n.device_id == Some(device.id) && matches!(n.node_type, NodeType::Sink))
            {
                sink_ids.push(sink_node.id);
                debug!("Found sink node to restore: {}", sink_node.name);
            } else {
                return Ok(None);
            }
        }

        if restoration.had_default_source {
            if let Some(source_node) = store
                .nodes
                .values()
                .find(|n| n.device_id == Some(device.id) && matches!(n.node_type, NodeType::Source))
            {
                source_ids.push(source_node.id);
                debug!("Found source node to restore: {}", source_node.name);
            } else {
                return Ok(None);
            }
        }

        Ok(Some((sink_ids, source_ids)))
    }

    fn is_usb_device(device_name: &str) -> bool {
        device_name.contains(".usb-") || device_name.contains("USB")
    }

    pub fn cleanup_expired(&mut self) {
        self.pending.retain(|device_name, restoration| {
            if restoration.is_expired() {
                debug!("Removing expired restoration for device {}", device_name);
                false
            } else {
                true
            }
        });
    }
}
