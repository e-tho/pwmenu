use log::{debug, error, info};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tokio::sync::watch;

use crate::pw::{
    devices::{Device, DeviceInternal},
    links::{Link, LinkInternal, Port, PortInternal},
    metadata::MetadataManager,
    nodes::{Node, NodeInternal},
};

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
        if let Some(metadata_manager) = &self.metadata_manager {
            if let Some(default_sink_name) = metadata_manager.get_default_sink() {
                debug!("Metadata default sink name: '{}'", default_sink_name);

                let mut found_default = false;
                for (node_id, node) in &mut self.nodes {
                    if matches!(node.node_type, crate::pw::nodes::NodeType::Sink) {
                        if node.name == default_sink_name {
                            if !node.is_default {
                                node.is_default = true;
                                self.default_sink = Some(*node_id);
                                found_default = true;
                                debug!("Set node {} as default sink from metadata", node_id);
                            }
                        } else if node.is_default {
                            node.is_default = false;
                        }
                    }
                }
                if !found_default {
                    debug!("Could not find sink node with name: {}", default_sink_name);
                }
            }

            if let Some(default_source_name) = metadata_manager.get_default_source() {
                debug!(
                    "Found default source from metadata: {}",
                    default_source_name
                );

                let mut found_default = false;
                for (node_id, node) in &mut self.nodes {
                    if matches!(node.node_type, crate::pw::nodes::NodeType::Source) {
                        if node.name == default_source_name {
                            if !node.is_default {
                                node.is_default = true;
                                self.default_source = Some(*node_id);
                                found_default = true;
                                debug!("Set node {} as default source from metadata", node_id);
                            }
                        } else if node.is_default {
                            node.is_default = false;
                        }
                    }
                }
                if !found_default {
                    debug!(
                        "Could not find source node with name: {}",
                        default_source_name
                    );
                }
            }
        }
    }
}

pub fn update_graph(store_rc: &Rc<RefCell<Store>>, graph_tx: &watch::Sender<AudioGraph>) {
    // Update metadata defaults before creating graph
    {
        let mut store = store_rc.borrow_mut();
        store.update_defaults_from_metadata();
    }

    let graph = store_rc.borrow().to_graph();
    if graph_tx.send(graph).is_err() {
        error!("Graph receiver dropped, cannot send updates.");
    }
}
