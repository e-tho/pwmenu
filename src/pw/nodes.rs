use libspa::{
    pod::builder::Builder,
    sys::{spa_pod_frame, SPA_PARAM_Props, SPA_PROP_mute, SPA_PROP_volume},
};
use pipewire::spa::{
    param::ParamType,
    pod::{deserialize::PodDeserializer, Pod, Value},
};
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, mem::MaybeUninit, rc::Rc};

use anyhow::{anyhow, Context as AnyhowContext, Result};
use log::{debug, error, warn};
use tokio::sync::watch;

use crate::pw::{
    graph::{AudioGraph, Store},
    volume::VolumeResolver,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    AudioSink,
    AudioSource,
    AudioDuplex,
    StreamOutputAudio,
    StreamInputAudio,
    AudioVirtual,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub linear: f32, // 0.0 - 1.0
    pub muted: bool,
}

impl Volume {
    pub fn new(linear: f32, muted: bool) -> Self {
        Self {
            linear: linear.clamp(0.0, 2.0),
            muted,
        }
    }

    pub fn percent(&self) -> u8 {
        (self.linear * 100.0).round() as u8
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: u32,
    pub name: String,
    pub nick: Option<String>,
    pub description: Option<String>,
    pub media_class: Option<String>,
    pub application_name: Option<String>,
    pub node_type: NodeType,
    pub volume: Volume,
    pub is_default: bool,
    pub device_id: Option<u32>,
    pub ports: Vec<u32>,
    pub media_name: Option<String>,
}

pub struct NodeInternal {
    pub id: u32,
    pub name: String,
    pub nick: Option<String>,
    pub description: Option<String>,
    pub media_class: Option<String>,
    pub application_name: Option<String>,
    pub node_type: NodeType,
    pub volume: f32,
    pub muted: bool,
    pub is_default: bool,
    pub device_id: Option<u32>,
    pub ports: Vec<u32>,
    pub proxy: pipewire::node::Node,
    pub listener: Option<pipewire::node::NodeListener>,
    pub info_listener: Option<pipewire::node::NodeListener>,
    pub has_received_params: bool,
    pub media_name: Option<String>,
}

impl NodeInternal {
    pub fn to_node(&self) -> Node {
        Node {
            id: self.id,
            name: self.name.clone(),
            nick: self.nick.clone(),
            description: self.description.clone(),
            media_class: self.media_class.clone(),
            application_name: self.application_name.clone(),
            node_type: self.node_type,
            volume: Volume::new(self.volume, self.muted),
            is_default: self.is_default,
            device_id: self.device_id,
            ports: self.ports.clone(),
            media_name: self.media_name.clone(),
        }
    }
}

impl Store {
    pub fn add_node(
        &mut self,
        registry: &Rc<pipewire::registry::Registry>,
        global: &pipewire::registry::GlobalObject<&pipewire::spa::utils::dict::DictRef>,
        store_rc: &Rc<RefCell<Store>>,
        graph_tx: &watch::Sender<AudioGraph>,
    ) -> Result<()> {
        let props = global
            .props
            .ok_or_else(|| anyhow!("Node {} has no props", global.id))?;
        let proxy = registry
            .bind::<pipewire::node::Node, &pipewire::spa::utils::dict::DictRef>(global)
            .with_context(|| format!("Failed to bind node {}", global.id))?;

        let name = props
            .get(*pipewire::keys::NODE_NAME)
            .or_else(|| props.get(*pipewire::keys::NODE_NICK))
            .unwrap_or("Unknown Node")
            .to_string();
        let nick = props.get(*pipewire::keys::NODE_NICK).map(str::to_string); // Add this line
        let description = props
            .get(*pipewire::keys::NODE_DESCRIPTION)
            .map(str::to_string);
        let application_name = props.get(*pipewire::keys::APP_NAME).map(str::to_string);
        let media_class = props.get(*pipewire::keys::MEDIA_CLASS).map(str::to_string);
        let device_id = props
            .get(*pipewire::keys::DEVICE_ID)
            .and_then(|id| id.parse().ok());

        let node_type = match media_class.as_deref() {
            Some("Audio/Sink") => NodeType::AudioSink,
            Some("Audio/Source") => NodeType::AudioSource,
            Some("Audio/Duplex") => NodeType::AudioDuplex,
            Some("Stream/Output/Audio") => NodeType::StreamOutputAudio,
            Some("Stream/Input/Audio") => NodeType::StreamInputAudio,
            _ => NodeType::Unknown,
        };

        let ports = self
            .ports
            .values()
            .filter(|p| p.node_id == global.id)
            .map(|p| p.id)
            .collect();

        let media_name = props.get("media.name").map(str::to_string);

        let mut node = NodeInternal {
            id: global.id,
            name: name.clone(),
            nick,
            description,
            media_class,
            application_name,
            node_type,
            volume: 1.0,
            muted: false,
            is_default: (node_type == NodeType::AudioSink && self.default_sink == Some(global.id))
                || (node_type == NodeType::AudioSource && self.default_source == Some(global.id)),
            device_id,
            ports,
            proxy,
            listener: None,
            info_listener: None,
            has_received_params: false,
            media_name,
        };

        let store_weak = Rc::downgrade(store_rc);
        let graph_tx_clone = graph_tx.clone();

        let listener = node
           .proxy
           .add_listener_local()
           .param({
               let store_weak = store_weak.clone();
               let graph_tx = graph_tx_clone.clone();
               let node_id = global.id;

               move |_seq, _param_type, _index, _next, pod_opt: Option<&pipewire::spa::pod::Pod>| {
                   if let Some(actual_pod) = pod_opt {
                       if let Some(upgraded_store_rc) = store_weak.upgrade() {
                           let updated = {
                               let mut store_borrow = match upgraded_store_rc.try_borrow_mut() {
                                   Ok(s) => s,
                                   Err(e) => {
                                       error!("Failed to borrow store in node param cb {node_id}: {e}");
                                       return;
                                   }
                               };
                               let result = store_borrow.update_node_param(node_id, actual_pod);

                               if result {
                                   if let Some(node) = store_borrow.nodes.get(&node_id) {
                                       if let Some(device_id) = node.device_id {
                                           if let Some(device) = store_borrow.devices.get(&device_id) {
                                               if device.has_route_volume {
                                                   device.proxy.enum_params(0, Some(ParamType::Route), 0, u32::MAX);
                                               }
                                           }
                                       }
                                   }
                               }

                               result
                           };
                           if updated {
                               crate::pw::graph::update_graph(&upgraded_store_rc, &graph_tx);
                           }
                       }
                   }
               }
           })
           .register();

        let info_listener = node
            .proxy
            .add_listener_local()
            .info({
                let store_weak = store_weak.clone();
                let graph_tx = graph_tx_clone.clone();
                let node_id = global.id;

                move |info| {
                    if let Some(store_rc) = store_weak.upgrade() {
                        let updated = match store_rc.try_borrow_mut() {
                            Ok(mut store) => {
                                if let Some(node) = store.nodes.get_mut(&node_id) {
                                    let mut node_updated = false;

                                    if let Some(props) = info.props() {
                                        if matches!(
                                            node.node_type,
                                            NodeType::StreamOutputAudio
                                                | NodeType::StreamInputAudio
                                        ) {
                                            if let Some(media_name) =
                                                props.get("media.name").map(str::to_string)
                                            {
                                                if node.media_name != Some(media_name.clone()) {
                                                    node.media_name = Some(media_name);
                                                    node_updated = true;
                                                }
                                            }
                                        }
                                    }

                                    node_updated
                                } else {
                                    false
                                }
                            }
                            Err(_) => false,
                        };

                        if updated {
                            crate::pw::graph::update_graph(&store_rc, &graph_tx);
                        }
                    }
                }
            })
            .register();

        node.listener = Some(listener);
        node.info_listener = Some(info_listener);

        node.proxy
            .subscribe_params(&[pipewire::spa::param::ParamType::Props]);

        self.nodes.insert(global.id, node);
        log::debug!("Added node {}: '{}'", global.id, name);

        if let Some(dev_id) = device_id {
            if let Some(device) = self.devices.get_mut(&dev_id) {
                if !device.nodes.contains(&global.id) {
                    device.nodes.push(global.id);
                }
            }
            self.update_device_type_from_nodes(dev_id);
        }
        Ok(())
    }

    pub fn update_node_param(&mut self, node_id: u32, pod: &Pod) -> bool {
        let Some(node) = self.nodes.get_mut(&node_id) else {
            return false;
        };

        let mut updated = false;

        if !node.has_received_params {
            node.has_received_params = true;
            updated = true;
        }

        if let Ok((_, Value::Object(obj))) = PodDeserializer::deserialize_any_from(pod.as_bytes()) {
            for prop in &obj.properties {
                match prop.key {
                    libspa::sys::SPA_PROP_channelVolumes => {
                        if matches!(node.node_type, NodeType::AudioSink | NodeType::AudioSource) {
                            if let Some(raw_volume) =
                                VolumeResolver::extract_channel_volume(&prop.value)
                            {
                                let scaled_volume = VolumeResolver::apply_cubic_scaling(raw_volume);
                                if (node.volume - scaled_volume).abs() > 0.001 {
                                    node.volume = scaled_volume;
                                    updated = true;
                                }
                            }
                        }
                    }
                    libspa::sys::SPA_PROP_volume => {
                        if let Value::Float(volume) = prop.value {
                            if (node.volume - volume).abs() > 0.001 {
                                node.volume = volume;
                                updated = true;
                            }
                        }
                    }
                    libspa::sys::SPA_PROP_mute => {
                        if let Value::Bool(mute) = prop.value {
                            if node.muted != mute {
                                node.muted = mute;
                                updated = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        updated
    }

    pub fn set_node_volume(&mut self, node_id: u32, volume: f32) -> Result<()> {
        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| anyhow!("Node {node_id} not found for set_node_volume"))?;

        let volume_value = volume.clamp(0.0, 2.0);

        let mut buffer: Vec<u8> = Vec::new();
        let mut builder = Builder::new(&mut buffer);
        let mut frame = MaybeUninit::<spa_pod_frame>::uninit();

        unsafe {
            builder
                .push_object(&mut frame, SPA_PARAM_Props, SPA_PARAM_Props)
                .context("Builder: failed to push object for volume")?;
            let initialized_frame = frame.assume_init_mut();
            builder
                .add_prop(SPA_PROP_volume, 0)
                .context("Builder: failed to add volume property key")?;
            builder
                .add_float(volume_value)
                .context("Builder: failed to add volume float value")?;
            builder.pop(initialized_frame);
        }

        let pod_ref = Pod::from_bytes(&buffer)
            .ok_or_else(|| anyhow!("Failed to create Pod reference from built bytes for volume"))?;

        node.proxy.set_param(ParamType::Props, 0, pod_ref);
        node.volume = volume_value;

        debug!("Sent volume command for node {node_id} to {volume_value}");
        Ok(())
    }

    pub fn set_node_mute(&mut self, node_id: u32, mute: bool) -> Result<()> {
        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| anyhow!("Node {node_id} not found for set_node_mute"))?;

        let mut buffer: Vec<u8> = Vec::new();
        let mut builder = Builder::new(&mut buffer);
        let mut frame = MaybeUninit::<spa_pod_frame>::uninit();

        unsafe {
            builder
                .push_object(&mut frame, SPA_PARAM_Props, SPA_PARAM_Props)
                .context("Builder: failed to push object for mute")?;
            let initialized_frame = frame.assume_init_mut();
            builder
                .add_prop(SPA_PROP_mute, 0)
                .context("Builder: failed to add mute property key")?;
            builder
                .add_bool(mute)
                .context("Builder: failed to add mute bool value")?;
            builder.pop(initialized_frame);
        }

        let pod_ref = Pod::from_bytes(&buffer)
            .ok_or_else(|| anyhow!("Failed to create Pod reference from built bytes for mute"))?;

        node.proxy.set_param(ParamType::Props, 0, pod_ref);
        node.muted = mute;

        debug!("Sent mute command for node {node_id} to {mute}");
        Ok(())
    }

    pub fn set_default_sink(&mut self, node_id: u32) -> Result<()> {
        let node = self
            .nodes
            .get(&node_id)
            .ok_or_else(|| anyhow!("Node {node_id} not found for set_default_sink"))?;

        if node.node_type != NodeType::AudioSink {
            return Err(anyhow!("Node {node_id} is not a Sink"));
        }
        if self.default_sink == Some(node_id) {
            return Ok(());
        }

        let node_name = node.name.clone();

        let old_default = self.default_sink.replace(node_id);
        debug!("Set default sink to node {node_id}");

        if let Some(old_id) = old_default {
            if let Some(old_node) = self.nodes.get_mut(&old_id) {
                old_node.is_default = false;
            }
        }

        if let Some(new_node) = self.nodes.get_mut(&node_id) {
            new_node.is_default = true;
        }

        if let Some(metadata_manager) = &self.metadata_manager {
            if metadata_manager.is_available() {
                if let Err(e) = metadata_manager.set_default_sink(&node_name) {
                    warn!("Failed to set system-wide default sink: {e}");
                } else {
                    debug!("System-wide default sink set successfully");
                }
            } else {
                debug!("Metadata manager not available, default not persisted system-wide");
            }
        }

        Ok(())
    }

    pub fn set_default_source(&mut self, node_id: u32) -> Result<()> {
        let node = self
            .nodes
            .get(&node_id)
            .ok_or_else(|| anyhow!("Node {node_id} not found for set_default_source"))?;

        if node.node_type != NodeType::AudioSource {
            return Err(anyhow!("Node {node_id} is not a Source"));
        }
        if self.default_source == Some(node_id) {
            return Ok(());
        }

        let node_name = node.name.clone();

        let old_default = self.default_source.replace(node_id);
        debug!("Set default source to node {node_id}");

        if let Some(old_id) = old_default {
            if let Some(old_node) = self.nodes.get_mut(&old_id) {
                old_node.is_default = false;
            }
        }

        if let Some(new_node) = self.nodes.get_mut(&node_id) {
            new_node.is_default = true;
        }

        if let Some(metadata_manager) = &self.metadata_manager {
            if metadata_manager.is_available() {
                if let Err(e) = metadata_manager.set_default_source(&node_name) {
                    warn!("Failed to set system-wide default source: {e}");
                } else {
                    debug!("System-wide default source set successfully");
                }
            } else {
                debug!("Metadata manager not available, default not persisted system-wide");
            }
        }

        Ok(())
    }

    pub fn get_output_nodes(&self) -> Vec<Node> {
        self.nodes
            .values()
            .filter(|n| matches!(n.node_type, NodeType::AudioSink))
            .map(|n| n.to_node())
            .collect()
    }

    pub fn get_input_nodes(&self) -> Vec<Node> {
        self.nodes
            .values()
            .filter(|n| matches!(n.node_type, NodeType::AudioSource))
            .map(|n| n.to_node())
            .collect()
    }

    pub fn get_node(&self, node_id: u32) -> Option<Node> {
        self.nodes.get(&node_id).map(|n| n.to_node())
    }
}
