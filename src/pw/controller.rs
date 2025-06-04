use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use crate::pw::{
    devices::{DeviceType, Profile},
    engine::PwEngine,
    nodes::{Node, NodeType},
};

pub struct Controller {
    engine: Arc<PwEngine>,
    log_sender: UnboundedSender<String>,
}

impl Controller {
    pub async fn new(log_sender: UnboundedSender<String>) -> Result<Self> {
        let engine = Arc::new(PwEngine::new().await?);

        try_send_log!(log_sender, "PipeWire controller initialized".to_string());

        Ok(Self { engine, log_sender })
    }

    pub fn get_output_nodes(&self) -> Vec<Node> {
        let graph = self.engine.graph();

        let mut nodes: Vec<Node> = graph
            .nodes
            .values()
            .filter(|n| {
                matches!(n.node_type, NodeType::Sink | NodeType::Duplex)
                    && !n.name.to_lowercase().contains("monitor")
            })
            .cloned()
            .collect();

        nodes.sort_by(|a, b| {
            b.is_default.cmp(&a.is_default).then_with(|| {
                let a_name = a.description.as_ref().unwrap_or(&a.name);
                let b_name = b.description.as_ref().unwrap_or(&b.name);
                a_name.cmp(b_name)
            })
        });

        nodes
    }

    pub fn get_input_nodes(&self) -> Vec<Node> {
        let graph = self.engine.graph();

        let mut nodes: Vec<Node> = graph
            .nodes
            .values()
            .filter(|n| {
                matches!(n.node_type, NodeType::Source | NodeType::Duplex)
                    && !n.name.to_lowercase().contains("monitor")
            })
            .cloned()
            .collect();

        nodes.sort_by(|a, b| {
            b.is_default.cmp(&a.is_default).then_with(|| {
                let a_name = a.description.as_ref().unwrap_or(&a.name);
                let b_name = b.description.as_ref().unwrap_or(&b.name);
                a_name.cmp(b_name)
            })
        });

        nodes
    }

    pub fn get_output_devices(&self) -> Vec<(u32, String)> {
        let graph = self.engine.graph();

        graph
            .devices
            .values()
            .filter(|d| d.device_type == DeviceType::Sink)
            .map(|d| (d.id, d.name.clone()))
            .collect()
    }

    pub fn get_input_devices(&self) -> Vec<(u32, String)> {
        let graph = self.engine.graph();

        graph
            .devices
            .values()
            .filter(|d| d.device_type == DeviceType::Source)
            .map(|d| (d.id, d.name.clone()))
            .collect()
    }

    pub fn get_node(&self, node_id: u32) -> Option<Node> {
        self.engine.graph().nodes.get(&node_id).cloned()
    }

    pub async fn set_node_volume(&self, node_id: u32, volume: f32) -> Result<()> {
        let result = self.engine.set_node_volume(node_id, volume).await;

        if result.is_ok() {
            if let Some(node) = self.get_node(node_id) {
                try_send_log!(
                    self.log_sender,
                    format!(
                        "Set volume for {} to {}%",
                        node.name,
                        (volume * 100.0) as u32
                    )
                );
            }
        }

        result
    }

    pub async fn set_node_mute(&self, node_id: u32, mute: bool) -> Result<()> {
        let result = self.engine.set_node_mute(node_id, mute).await;

        if result.is_ok() {
            if let Some(node) = self.get_node(node_id) {
                try_send_log!(
                    self.log_sender,
                    format!("{} {}", if mute { "Muted" } else { "Unmuted" }, node.name)
                );
            }
        }

        result
    }

    pub async fn create_link(&self, output_node: u32, input_node: u32) -> Result<()> {
        let result = self.engine.create_link(output_node, input_node).await;

        if result.is_ok() {
            let graph = self.engine.graph();
            let output_name = graph.nodes.get(&output_node).map_or("unknown", |n| &n.name);
            let input_name = graph.nodes.get(&input_node).map_or("unknown", |n| &n.name);

            try_send_log!(
                self.log_sender,
                format!("Created link from {} to {}", output_name, input_name)
            );
        }

        result
    }

    pub async fn remove_link(&self, output_node: u32, input_node: u32) -> Result<()> {
        let result = self.engine.remove_link(output_node, input_node).await;

        if result.is_ok() {
            let graph = self.engine.graph();
            let output_name = graph.nodes.get(&output_node).map_or("unknown", |n| &n.name);
            let input_name = graph.nodes.get(&input_node).map_or("unknown", |n| &n.name);

            try_send_log!(
                self.log_sender,
                format!("Removed link from {} to {}", output_name, input_name)
            );
        }

        result
    }

    pub async fn set_default_sink(&self, node_id: u32) -> Result<()> {
        let result = self.engine.set_default_sink(node_id).await;

        if result.is_ok() {
            if let Some(node) = self.get_node(node_id) {
                try_send_log!(
                    self.log_sender,
                    format!("Set default output to {}", node.name)
                );
            }
        }

        result
    }

    pub async fn set_default_source(&self, node_id: u32) -> Result<()> {
        let result = self.engine.set_default_source(node_id).await;

        if result.is_ok() {
            if let Some(node) = self.get_node(node_id) {
                try_send_log!(
                    self.log_sender,
                    format!("Set default input to {}", node.name)
                );
            }
        }

        result
    }

    pub fn get_default_sink(&self) -> Option<u32> {
        self.engine.graph().default_sink
    }

    pub fn get_default_source(&self) -> Option<u32> {
        self.engine.graph().default_source
    }

    pub fn get_device_profiles(&self, device_id: u32) -> Vec<Profile> {
        let graph = self.engine.graph();
        graph
            .devices
            .get(&device_id)
            .map(|device| device.profiles.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.is_available() && !p.is_off())
            .collect()
    }

    pub fn get_device_current_profile(&self, device_id: u32) -> Option<Profile> {
        let graph = self.engine.graph();
        graph.devices.get(&device_id).and_then(|device| {
            device
                .current_profile_index
                .and_then(|index| device.profiles.iter().find(|p| p.index == index).cloned())
        })
    }

    pub fn get_device_name(&self, device_id: u32) -> String {
        self.engine
            .graph()
            .devices
            .get(&device_id)
            .map(|d| d.description.as_ref().unwrap_or(&d.name).clone())
            .unwrap_or_else(|| "Unknown Device".to_string())
    }

    pub async fn switch_device_profile(&self, device_id: u32, profile_index: u32) -> Result<()> {
        let result = self
            .engine
            .switch_device_profile_with_restoration(device_id, profile_index)
            .await;

        if result.is_ok() {
            if let Some(device) = self.engine.graph().devices.get(&device_id) {
                if let Some(profile) = device.profiles.iter().find(|p| p.index == profile_index) {
                    try_send_log!(
                        self.log_sender,
                        format!(
                            "Switched device {} to profile: {}",
                            device.name, profile.description
                        )
                    );
                }
            }
        }

        result
    }
}
