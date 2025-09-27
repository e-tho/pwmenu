use crate::pw::{
    devices::{Device, DeviceInternal},
    links::{Link, LinkInternal, Port, PortInternal},
    metadata::MetadataManager,
    nodes::{Node, NodeInternal},
    restoration::RestorationManager,
    DeviceType, NodeType,
};
use anyhow::anyhow;
use anyhow::Result;
use log::{debug, error, warn};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ConnectionStatus {
    Connected,
    #[default]
    Disconnected,
    Error,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct AudioGraph {
    pub nodes: HashMap<u32, Node>,
    pub devices: HashMap<u32, Device>,
    pub ports: HashMap<u32, Port>,
    pub links: HashMap<u32, Link>,
    pub default_sink: Option<u32>,
    pub default_source: Option<u32>,
    pub connection_status: ConnectionStatus,
    pub initial_sync_complete: bool,
    pub params_sync_complete: bool,
    pub data_complete: bool,
    pub default_clock_rate: u32,
}

pub struct Store {
    pub nodes: HashMap<u32, NodeInternal>,
    pub devices: HashMap<u32, DeviceInternal>,
    pub ports: HashMap<u32, PortInternal>,
    pub links: HashMap<u32, LinkInternal>,
    pub default_sink: Option<u32>,
    pub default_source: Option<u32>,
    pub connection_status: ConnectionStatus,
    pub pwmenu_client_id: Option<u32>,
    pub core: Rc<pipewire::core::CoreRc>,
    pub metadata_manager: Option<MetadataManager>,
    pub restoration_manager: RestorationManager,
    pub initial_sync_complete: bool,
    pub initial_sync_seq: Option<i32>,
    pub params_sync_complete: bool,
    pub params_sync_seq: Option<i32>,
    pub data_complete: bool,
    pub refresh_pending: bool,
    pub default_clock_rate: u32,
}

impl Store {
    pub fn new(core: Rc<pipewire::core::CoreRc>) -> Self {
        Self {
            nodes: HashMap::new(),
            devices: HashMap::new(),
            ports: HashMap::new(),
            links: HashMap::new(),
            default_sink: None,
            default_source: None,
            connection_status: ConnectionStatus::Connected,
            pwmenu_client_id: None,
            core,
            metadata_manager: None,
            restoration_manager: RestorationManager::new(),
            initial_sync_complete: false,
            initial_sync_seq: None,
            params_sync_complete: false,
            params_sync_seq: None,
            data_complete: false,
            refresh_pending: false,
            default_clock_rate: 48000,
        }
    }

    pub fn to_graph(&self) -> AudioGraph {
        AudioGraph {
            nodes: self
                .nodes
                .iter()
                .map(|(&id, node)| (id, node.to_node()))
                .collect(),
            devices: self
                .devices
                .iter()
                .map(|(&id, device)| (id, device.to_device()))
                .collect(),
            ports: self
                .ports
                .iter()
                .map(|(&id, port)| (id, port.to_port()))
                .collect(),
            links: self
                .links
                .iter()
                .map(|(&id, link)| (id, link.to_link()))
                .collect(),
            default_sink: self.default_sink,
            default_source: self.default_source,
            connection_status: self.connection_status,
            initial_sync_complete: self.initial_sync_complete,
            params_sync_complete: self.params_sync_complete,
            data_complete: self.data_complete,
            default_clock_rate: self.default_clock_rate,
        }
    }

    pub fn handle_sync_done(&mut self, seq: i32) {
        debug!(
            "Handling sync done: received seq={}, expecting initial={:?}, params={:?}",
            seq, self.initial_sync_seq, self.params_sync_seq
        );

        if let Some(initial_seq) = self.initial_sync_seq {
            if seq == initial_seq && !self.initial_sync_complete {
                self.initial_sync_complete = true;
                debug!("Initial sync complete! (seq: {seq})");
                return;
            }
        }

        if let Some(params_seq) = self.params_sync_seq {
            if seq == params_seq && !self.params_sync_complete {
                self.params_sync_complete = true;
                debug!("Parameter sync complete! (seq: {seq})");
                return;
            }
        }

        debug!("Received sync done for untracked sequence: {seq}");
    }

    pub fn setup_metadata_manager(
        &mut self,
        store_rc: &Rc<RefCell<Store>>,
        graph_tx: &watch::Sender<AudioGraph>,
    ) {
        let store_weak = Rc::downgrade(store_rc);
        let graph_tx_clone = graph_tx.clone();

        let update_callback = move || {
            if let Some(store_rc) = store_weak.upgrade() {
                update_graph(&store_rc, &graph_tx_clone);
            }
        };

        self.metadata_manager = Some(MetadataManager::new().with_update_callback(update_callback));
    }

    pub fn set_pwmenu_client_id(&mut self, id: u32) {
        self.pwmenu_client_id = Some(id);
        debug!("Internal PipeWire client ID set to: {id}");
    }

    pub fn update_defaults_from_metadata(&mut self) {
        let Some(metadata_manager) = &self.metadata_manager else {
            return;
        };

        if let Some(default_sink_name) = metadata_manager.get_default_sink() {
            let mut found_default = false;

            for (node_id, node) in &mut self.nodes {
                if !matches!(node.node_type, crate::pw::nodes::NodeType::AudioSink) {
                    continue;
                }

                let name_matches = node.name == default_sink_name
                    || node.name.trim() == default_sink_name.trim()
                    || node.description.as_ref() == Some(&default_sink_name)
                    || node
                        .description
                        .as_ref()
                        .is_some_and(|desc| desc.trim() == default_sink_name.trim());

                if name_matches {
                    if !node.is_default {
                        node.is_default = true;
                        self.default_sink = Some(*node_id);
                        debug!("Set node {node_id} as default sink from metadata");
                    }
                    found_default = true;
                } else if node.is_default {
                    node.is_default = false;
                }
            }

            // Fallback: if only one sink exists, assume it's default
            if !found_default {
                let sink_node_ids: Vec<u32> = self
                    .nodes
                    .iter()
                    .filter(|(_, n)| matches!(n.node_type, crate::pw::nodes::NodeType::AudioSink))
                    .map(|(id, _)| *id)
                    .collect();

                if sink_node_ids.len() == 1 {
                    if let Some(node) = self.nodes.get_mut(&sink_node_ids[0]) {
                        node.is_default = true;
                        self.default_sink = Some(sink_node_ids[0]);
                    }
                }
            }
        }

        // Similar logic for source (simplified for brevity)
        if let Some(default_source_name) = metadata_manager.get_default_source() {
            for (node_id, node) in &mut self.nodes {
                if matches!(node.node_type, crate::pw::nodes::NodeType::AudioSource) {
                    let name_matches = node.name == default_source_name
                        || node.name.trim() == default_source_name.trim()
                        || node.description.as_ref() == Some(&default_source_name);

                    if name_matches {
                        if !node.is_default {
                            node.is_default = true;
                            self.default_source = Some(*node_id);
                            debug!("Set node {node_id} as default source from metadata");
                        }
                    } else if node.is_default {
                        node.is_default = false;
                    }
                }
            }
        }
    }

    pub fn switch_device_profile_with_restoration(
        &mut self,
        device_id: u32,
        profile_index: u32,
    ) -> Result<()> {
        if let Some((device_name, had_default_sink, had_default_source)) =
            RestorationManager::should_capture_defaults(self, device_id)
        {
            self.restoration_manager.capture_defaults(
                device_id,
                device_name,
                had_default_sink,
                had_default_source,
                profile_index,
            );
        }

        self.switch_device_profile(device_id, profile_index)
    }

    fn check_data_completeness(&mut self) -> bool {
        if self.devices.is_empty() {
            return false;
        }

        let has_audio_nodes = self
            .nodes
            .values()
            .any(|n| matches!(n.node_type, NodeType::AudioSink | NodeType::AudioSource));

        if !has_audio_nodes {
            return false;
        }

        let all_audio_nodes_have_params = self
            .nodes
            .values()
            .filter(|n| matches!(n.node_type, NodeType::AudioSink | NodeType::AudioSource))
            .all(|n| n.has_received_params);

        if !all_audio_nodes_have_params {
            return false;
        }

        let device_ids: Vec<u32> = self.devices.keys().copied().collect();
        for device_id in device_ids {
            self.update_device_type_from_nodes(device_id);
        }

        let mut found_audio_device = false;
        for device in self.devices.values() {
            if device.device_type == DeviceType::Unknown {
                continue;
            }

            found_audio_device = true;

            if device.profiles.is_empty() {
                return false;
            }

            if device.current_profile_index.is_none() {
                return false;
            }
        }

        if !found_audio_device {
            return false;
        }

        self.apply_default_fallbacks();

        true
    }

    fn apply_default_fallbacks(&mut self) {
        let has_sinks = self
            .nodes
            .values()
            .any(|n| matches!(n.node_type, NodeType::AudioSink));
        let has_sources = self
            .nodes
            .values()
            .any(|n| matches!(n.node_type, NodeType::AudioSource));

        if has_sinks && self.default_sink.is_none() {
            let sink_ids: Vec<u32> = self
                .nodes
                .iter()
                .filter(|(_, n)| matches!(n.node_type, NodeType::AudioSink))
                .map(|(id, _)| *id)
                .collect();

            if sink_ids.len() == 1 {
                self.default_sink = Some(sink_ids[0]);
                if let Some(node) = self.nodes.get_mut(&sink_ids[0]) {
                    node.is_default = true;
                }
            }
        }

        if has_sources && self.default_source.is_none() {
            let source_ids: Vec<u32> = self
                .nodes
                .iter()
                .filter(|(_, n)| matches!(n.node_type, NodeType::AudioSource))
                .map(|(id, _)| *id)
                .collect();

            if source_ids.len() == 1 {
                self.default_source = Some(source_ids[0]);
                if let Some(node) = self.nodes.get_mut(&source_ids[0]) {
                    node.is_default = true;
                }
            }
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) -> Result<()> {
        self.default_clock_rate = sample_rate;

        if let Some(metadata_manager) = &self.metadata_manager {
            metadata_manager.set_sample_rate(sample_rate)?;
        } else {
            return Err(anyhow!("Metadata manager not available"));
        }

        debug!("Set global sample rate to {} Hz", sample_rate);
        Ok(())
    }
}

pub fn update_graph(store_rc: &Rc<RefCell<Store>>, graph_tx: &watch::Sender<AudioGraph>) {
    let (nodes_to_restore, completed_devices) = {
        let store = store_rc.borrow();
        store.restoration_manager.get_pending_restorations(&store)
    };

    {
        let mut store = store_rc.borrow_mut();
        store.update_defaults_from_metadata();

        if !store.data_complete {
            store.data_complete = store.check_data_completeness();
        }

        store.restoration_manager.update_attempts_and_cleanup();
        store.restoration_manager.mark_completed(&completed_devices);
        store.restoration_manager.cleanup_expired();
    }

    if !nodes_to_restore.is_empty() {
        let mut store = store_rc.borrow_mut();
        for (sink_id, source_id) in nodes_to_restore {
            if sink_id != 0 {
                if let Err(e) = store.set_default_sink(sink_id) {
                    warn!("Failed to restore default sink {sink_id}: {e}");
                } else {
                    debug!("Restored default sink: {sink_id}");
                }
            }
            if source_id != 0 {
                if let Err(e) = store.set_default_source(source_id) {
                    warn!("Failed to restore default source {source_id}: {e}");
                } else {
                    debug!("Restored default source: {source_id}");
                }
            }
        }
    }

    let graph = store_rc.borrow().to_graph();
    if graph_tx.send(graph).is_err() {
        error!("Graph receiver dropped, cannot send updates.");
    }
}
