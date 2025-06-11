use crate::pw::{
    devices::{Device, DeviceInternal},
    links::{Link, LinkInternal, Port, PortInternal},
    metadata::MetadataManager,
    nodes::{Node, NodeInternal},
    restoration::RestorationManager,
};
use anyhow::Result;
use log::{debug, error, info, warn};
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
    pub core: Rc<pipewire::core::Core>,
    pub metadata_manager: Option<MetadataManager>,
    pub restoration_manager: RestorationManager,
}

impl Store {
    pub fn new(core: Rc<pipewire::core::Core>) -> Self {
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
        }
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
        }
    }

    pub fn set_pwmenu_client_id(&mut self, id: u32) {
        self.pwmenu_client_id = Some(id);
        info!("Internal PipeWire client ID set to: {}", id);
    }

    pub fn update_defaults_from_metadata(&mut self) {
        let Some(metadata_manager) = &self.metadata_manager else {
            return;
        };

        if let Some(default_sink_name) = metadata_manager.get_default_sink() {
            let mut found_default = false;

            for (node_id, node) in &mut self.nodes {
                if !matches!(node.node_type, crate::pw::nodes::NodeType::Sink) {
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
                        debug!("Set node {} as default sink from metadata", node_id);
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
                    .filter(|(_, n)| matches!(n.node_type, crate::pw::nodes::NodeType::Sink))
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
                if matches!(node.node_type, crate::pw::nodes::NodeType::Source) {
                    let name_matches = node.name == default_source_name
                        || node.name.trim() == default_source_name.trim()
                        || node.description.as_ref() == Some(&default_source_name);

                    if name_matches {
                        if !node.is_default {
                            node.is_default = true;
                            self.default_source = Some(*node_id);
                            debug!("Set node {} as default source from metadata", node_id);
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
}

pub fn update_graph(store_rc: &Rc<RefCell<Store>>, graph_tx: &watch::Sender<AudioGraph>) {
    let (nodes_to_restore, completed_devices) = {
        let store = store_rc.borrow();
        store.restoration_manager.get_pending_restorations(&store)
    };

    {
        let mut store = store_rc.borrow_mut();
        store.update_defaults_from_metadata();
        store.restoration_manager.update_attempts_and_cleanup();
        store.restoration_manager.mark_completed(&completed_devices);
        store.restoration_manager.cleanup_expired();
    }

    if !nodes_to_restore.is_empty() {
        let mut store = store_rc.borrow_mut();
        for (sink_id, source_id) in nodes_to_restore {
            if sink_id != 0 {
                if let Err(e) = store.set_default_sink(sink_id) {
                    warn!("Failed to restore default sink {}: {}", sink_id, e);
                } else {
                    debug!("Restored default sink: {}", sink_id);
                }
            }
            if source_id != 0 {
                if let Err(e) = store.set_default_source(source_id) {
                    warn!("Failed to restore default source {}: {}", source_id, e);
                } else {
                    debug!("Restored default source: {}", source_id);
                }
            }
        }
    }

    let graph = store_rc.borrow().to_graph();
    if graph_tx.send(graph).is_err() {
        error!("Graph receiver dropped, cannot send updates.");
    }
}
