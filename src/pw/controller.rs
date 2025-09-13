use anyhow::{anyhow, Result};
use log::{debug, info};
use std::sync::Arc;

use crate::pw::{
    devices::{DeviceType, Profile},
    engine::PwEngine,
    nodes::{Node, NodeType, Volume},
    volume::RouteDirection,
    AudioGraph,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FormFactorPriority {
    Headphones = 0,
    Headset = 1,
    HandsFree = 2,
    Handset = 3,
    Speaker = 4,
    Microphone = 5,
    Webcam = 6,
    Portable = 7,
    Car = 8,
    Hifi = 9,
    Tv = 10,
    Computer = 11,
    Internal = 12,
    Unknown = 13,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BusPriority {
    Usb = 0,
    Bluetooth = 1,
    Pci = 2,
    Unknown = 3,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub nick: Option<String>,
    pub form_factor: Option<String>,
    pub bus: Option<String>,
    pub media_class: Option<String>,
    pub is_muted: bool,
    pub node_type: NodeType,
}

pub struct Controller {
    engine: Arc<PwEngine>,
}

impl Controller {
    pub async fn new() -> Result<Self> {
        let engine = Arc::new(PwEngine::new().await?);

        info!("{}", t!("notifications.pw.initialized"));

        Ok(Self { engine })
    }

    pub async fn wait_for_initialization(&self) -> Result<()> {
        self.engine.wait_for_initialization().await
    }

    pub fn get_output_nodes(&self) -> Vec<Node> {
        let graph = self.engine.graph();

        let nodes: Vec<Node> = graph
            .nodes
            .values()
            .filter(|n| matches!(n.node_type, NodeType::AudioSink))
            .map(|n| self.enhance_node_volume(n, &graph))
            .collect();

        self.sort_nodes_by_priority(nodes)
    }

    pub fn get_input_nodes(&self) -> Vec<Node> {
        let graph = self.engine.graph();

        let nodes: Vec<Node> = graph
            .nodes
            .values()
            .filter(|n| matches!(n.node_type, NodeType::AudioSource))
            .map(|n| self.enhance_node_volume(n, &graph))
            .collect();

        self.sort_nodes_by_priority(nodes)
    }

    pub fn get_output_streams(&self) -> Vec<Node> {
        let graph = self.engine.graph();

        graph
            .nodes
            .values()
            .filter(|n| matches!(n.node_type, NodeType::StreamOutputAudio))
            .map(|n| self.enhance_node_volume(n, &graph))
            .collect()
    }

    pub fn get_input_streams(&self) -> Vec<Node> {
        let graph = self.engine.graph();

        graph
            .nodes
            .values()
            .filter(|n| matches!(n.node_type, NodeType::StreamInputAudio))
            .map(|n| self.enhance_node_volume(n, &graph))
            .collect()
    }

    pub fn get_node(&self, node_id: u32) -> Option<Node> {
        let graph = self.engine.graph();
        let node = graph.nodes.get(&node_id)?;
        Some(self.enhance_node_volume(node, &graph))
    }

    fn enhance_node_volume(&self, node: &Node, graph: &AudioGraph) -> Node {
        if let Some(device_id) = node.device_id {
            if let Some(device) = graph.devices.get(&device_id) {
                if device.has_route_volume {
                    let route_direction = match node.node_type {
                        NodeType::AudioSink => Some(RouteDirection::Output),
                        NodeType::AudioSource => Some(RouteDirection::Input),
                        _ => None,
                    };

                    if let Some(direction) = route_direction {
                        if let Some((route_volume, route_muted)) =
                            self.get_cached_route_volume(device, direction)
                        {
                            let mut enhanced_node = node.clone();
                            enhanced_node.volume = Volume::new(route_volume, route_muted);
                            return enhanced_node;
                        }
                    }
                }
            }
        }
        node.clone()
    }

    fn get_cached_route_volume(
        &self,
        device: &crate::pw::devices::Device,
        direction: RouteDirection,
    ) -> Option<(f32, bool)> {
        match direction {
            RouteDirection::Output => device.output_route.get_volume_state(),
            RouteDirection::Input => device.input_route.get_volume_state(),
        }
    }

    fn sort_nodes_by_priority(&self, mut nodes: Vec<Node>) -> Vec<Node> {
        let graph = self.engine.graph();

        nodes.sort_by(|a, b| {
            b.is_default
                .cmp(&a.is_default)
                .then_with(|| {
                    let a_form_factor = self.get_form_factor_priority(a, &graph);
                    let b_form_factor = self.get_form_factor_priority(b, &graph);
                    a_form_factor.cmp(&b_form_factor)
                })
                .then_with(|| {
                    let a_bus = self.get_bus_priority(a, &graph);
                    let b_bus = self.get_bus_priority(b, &graph);
                    a_bus.cmp(&b_bus)
                })
                .then_with(|| {
                    a.description
                        .as_ref()
                        .unwrap_or(&a.name)
                        .cmp(b.description.as_ref().unwrap_or(&b.name))
                })
        });
        nodes
    }

    fn get_form_factor_priority(&self, node: &Node, graph: &AudioGraph) -> FormFactorPriority {
        if let Some(device_id) = node.device_id {
            if let Some(device) = graph.devices.get(&device_id) {
                match device.form_factor.as_deref() {
                    Some("headphone") => FormFactorPriority::Headphones,
                    Some("headset") => FormFactorPriority::Headset,
                    Some("hands-free") => FormFactorPriority::HandsFree,
                    Some("handset") => FormFactorPriority::Handset,
                    Some("speaker") => FormFactorPriority::Speaker,
                    Some("microphone") => FormFactorPriority::Microphone,
                    Some("webcam") => FormFactorPriority::Webcam,
                    Some("portable") => FormFactorPriority::Portable,
                    Some("car") => FormFactorPriority::Car,
                    Some("hifi") => FormFactorPriority::Hifi,
                    Some("tv") => FormFactorPriority::Tv,
                    Some("computer") => FormFactorPriority::Computer,
                    Some("internal") => FormFactorPriority::Internal,
                    _ => FormFactorPriority::Unknown,
                }
            } else {
                FormFactorPriority::Unknown
            }
        } else {
            FormFactorPriority::Unknown
        }
    }

    fn get_bus_priority(&self, node: &Node, graph: &AudioGraph) -> BusPriority {
        if let Some(device_id) = node.device_id {
            if let Some(device) = graph.devices.get(&device_id) {
                match device.bus.as_deref() {
                    Some("bluetooth") => BusPriority::Bluetooth,
                    Some("pci") => BusPriority::Pci,
                    Some("usb") => BusPriority::Usb,
                    _ => BusPriority::Unknown,
                }
            } else {
                BusPriority::Unknown
            }
        } else {
            BusPriority::Unknown
        }
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

    pub async fn set_volume(&self, node_id: u32, volume: f32) -> Result<()> {
        let graph = self.engine.graph();
        let node = graph
            .nodes
            .get(&node_id)
            .ok_or_else(|| anyhow!("Node {node_id} not found"))?;

        let result = match node.node_type {
            NodeType::StreamOutputAudio | NodeType::StreamInputAudio => {
                self.engine.set_node_volume(node_id, volume).await
            }
            NodeType::AudioSink | NodeType::AudioSource => {
                if let Some(device_id) = node.device_id {
                    if let Some(device) = graph.devices.get(&device_id) {
                        if device.has_route_volume {
                            let target_direction = match node.node_type {
                                NodeType::AudioSink | NodeType::AudioDuplex => {
                                    Some(RouteDirection::Output)
                                }
                                NodeType::AudioSource => Some(RouteDirection::Input),
                                _ => None,
                            };

                            if let Some(direction) = target_direction {
                                match self
                                    .engine
                                    .set_device_volume(device_id, volume, Some(direction))
                                    .await
                                {
                                    Ok(()) => Ok(()),
                                    Err(_) => self.engine.set_node_volume(node_id, volume).await,
                                }
                            } else {
                                self.engine.set_node_volume(node_id, volume).await
                            }
                        } else {
                            self.engine.set_node_volume(node_id, volume).await
                        }
                    } else {
                        self.engine.set_node_volume(node_id, volume).await
                    }
                } else {
                    self.engine.set_node_volume(node_id, volume).await
                }
            }
            _ => self.engine.set_node_volume(node_id, volume).await,
        };

        if result.is_ok() {
            info!(
                "Set volume for {} to {}%",
                node.description.as_ref().unwrap_or(&node.name),
                (volume * 100.0) as u32
            );
        }

        result
    }

    pub async fn set_mute(&self, node_id: u32, mute: bool) -> Result<()> {
        let graph = self.engine.graph();
        let node = graph
            .nodes
            .get(&node_id)
            .ok_or_else(|| anyhow!("Node {node_id} not found"))?;

        // Try device-level control first, fall back to node-level
        let result = if let Some(device_id) = node.device_id {
            if let Some(device) = graph.devices.get(&device_id) {
                if device.has_route_volume {
                    let target_direction = match node.node_type {
                        NodeType::AudioSink => {
                            if device.output_route.is_available() {
                                Some(RouteDirection::Output)
                            } else {
                                None
                            }
                        }
                        NodeType::AudioSource => {
                            if device.input_route.is_available() {
                                Some(RouteDirection::Input)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };

                    if let Some(direction) = target_direction {
                        match self
                            .engine
                            .set_device_mute(device_id, mute, Some(direction))
                            .await
                        {
                            Ok(()) => Ok(()),
                            Err(_) => self.engine.set_node_mute(node_id, mute).await,
                        }
                    } else {
                        self.engine.set_node_mute(node_id, mute).await
                    }
                } else {
                    self.engine.set_node_mute(node_id, mute).await
                }
            } else {
                self.engine.set_node_mute(node_id, mute).await
            }
        } else {
            self.engine.set_node_mute(node_id, mute).await
        };

        if result.is_ok() {
            info!(
                "{} {}",
                if mute { "Muted" } else { "Unmuted" },
                node.description.as_ref().unwrap_or(&node.name)
            );
        }

        result
    }

    fn extract_display_name_from_identifier(identifier: &str) -> String {
        if identifier.contains('.') {
            identifier
                .split('.')
                .next_back()
                .unwrap_or(identifier)
                .to_string()
        } else {
            identifier.to_string()
        }
    }

    pub fn get_application_name(&self, node: &Node) -> String {
        if let Some(app_name) = &node.application_name {
            return Self::extract_display_name_from_identifier(app_name);
        }

        if let Some(nick) = &node.nick {
            return nick.clone();
        }

        if let Some(desc) = &node.description {
            return Self::extract_display_name_from_identifier(desc);
        }

        node.name.clone()
    }

    pub fn get_media_name(&self, node: &Node) -> Option<String> {
        node.media_name.clone()
    }

    pub async fn create_link(&self, output_node: u32, input_node: u32) -> Result<()> {
        let result = self.engine.create_link(output_node, input_node).await;

        if result.is_ok() {
            let graph = self.engine.graph();
            let output_name = graph.nodes.get(&output_node).map_or("unknown", |n| &n.name);
            let input_name = graph.nodes.get(&input_node).map_or("unknown", |n| &n.name);

            debug!("Created link from {output_name} to {input_name}");
        }

        result
    }

    pub async fn remove_link(&self, output_node: u32, input_node: u32) -> Result<()> {
        let result = self.engine.remove_link(output_node, input_node).await;

        if result.is_ok() {
            let graph = self.engine.graph();
            let output_name = graph.nodes.get(&output_node).map_or("unknown", |n| &n.name);
            let input_name = graph.nodes.get(&input_node).map_or("unknown", |n| &n.name);

            debug!("Removed link from {output_name} to {input_name}");
        }

        result
    }

    pub async fn set_default_sink(&self, node_id: u32) -> Result<()> {
        let result = self.engine.set_default_sink(node_id).await;

        if result.is_ok() {
            if let Some(node) = self.get_node(node_id) {
                debug!("Set default output to {}", node.name);
            }
        }

        result
    }

    pub async fn set_default_source(&self, node_id: u32) -> Result<()> {
        let result = self.engine.set_default_source(node_id).await;

        if result.is_ok() {
            if let Some(node) = self.get_node(node_id) {
                debug!("Set default input to {}", node.name);
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

    pub fn get_device_info(&self, node: &Node) -> DeviceInfo {
        let mut device_info = DeviceInfo {
            nick: None,
            form_factor: None,
            bus: None,
            media_class: node.media_class.clone(),
            is_muted: node.volume.muted,
            node_type: node.node_type,
        };

        if let Some(device_id) = node.device_id {
            let graph = self.engine.graph();
            if let Some(device) = graph.devices.get(&device_id) {
                device_info.nick = device.nick.clone();
                device_info.form_factor = device.form_factor.clone();
                device_info.bus = device.bus.clone();
            }
        }

        device_info
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
            .map(|d| {
                d.nick
                    .as_ref()
                    .or(d.description.as_ref())
                    .unwrap_or(&d.name)
                    .clone()
            })
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
                    debug!(
                        "Switched device {} to profile: {}",
                        device.name, profile.description
                    );
                }
            }
        }

        result
    }

    pub fn get_node_base_name(&self, node: &Node) -> String {
        self.get_device_info(node)
            .nick
            .as_ref()
            .or(node.description.as_ref())
            .unwrap_or(&node.name)
            .to_string()
    }

    pub fn get_node_port_number(&self, node: &Node) -> Option<usize> {
        let nodes_of_same_type = match node.node_type {
            NodeType::AudioSink => self.get_output_nodes(),
            NodeType::AudioSource => self.get_input_nodes(),
            _ => return None,
        }
        .into_iter()
        .filter(|n| n.device_id == node.device_id)
        .collect::<Vec<_>>();

        if nodes_of_same_type.len() <= 1 {
            return None;
        }

        let graph = self.engine.graph();
        let node_ports = graph
            .ports
            .values()
            .filter(|p| p.node_id == node.id)
            .collect::<Vec<_>>();

        if node_ports.is_empty() {
            return None;
        }

        let mut all_ports = nodes_of_same_type
            .iter()
            .flat_map(|n| graph.ports.values().filter(|p| p.node_id == n.id))
            .collect::<Vec<_>>();

        all_ports.sort_by(|a, b| a.id.cmp(&b.id));

        if let Some(pos) = all_ports.iter().position(|p| p.id == node_ports[0].id) {
            return Some(pos + 1);
        }

        None
    }
}
