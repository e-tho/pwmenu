use anyhow::{anyhow, Context as AnyhowContext, Result};
use log::{debug, error, warn};
use pipewire::{
    keys::*, properties::properties, registry::GlobalObject, spa::utils::dict::DictRef,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, rc::Rc};

use crate::pw::graph::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    pub id: u32,
    pub name: String,
    pub node_id: u32,
    pub direction: PortDirection,
    pub channel: String,
    pub links: Vec<u32>,
}

#[derive(Debug)]
pub struct PortInternal {
    pub id: u32,
    pub name: String,
    pub node_id: u32,
    pub direction: PortDirection,
    pub channel: String,
    pub links: Vec<u32>,
    pub proxy: pipewire::port::Port,
}

impl PortInternal {
    pub fn to_port(&self) -> Port {
        Port {
            id: self.id,
            name: self.name.clone(),
            node_id: self.node_id,
            direction: self.direction,
            channel: self.channel.clone(),
            links: self.links.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub id: u32,
    pub output_node: u32,
    pub output_port: u32,
    pub input_node: u32,
    pub input_port: u32,
}

#[derive(Debug)]
pub struct LinkInternal {
    pub id: u32,
    pub output_node: u32,
    pub output_port: u32,
    pub input_node: u32,
    pub input_port: u32,
    pub proxy: pipewire::link::Link,
}

impl LinkInternal {
    pub fn to_link(&self) -> Link {
        Link {
            id: self.id,
            output_node: self.output_node,
            output_port: self.output_port,
            input_node: self.input_node,
            input_port: self.input_port,
        }
    }
}

impl Store {
    pub fn add_port(
        &mut self,
        registry: &Rc<pipewire::registry::Registry>,
        global: &GlobalObject<&DictRef>,
    ) -> Result<()> {
        let props = global
            .props
            .ok_or_else(|| anyhow!("Port {} has no props", global.id))?;
        let proxy = registry
            .bind::<pipewire::port::Port, &DictRef>(global)
            .with_context(|| format!("Failed to bind port {}", global.id))?;

        let name = props.get(*PORT_NAME).unwrap_or("Unknown Port").to_string();
        let node_id_str = props
            .get(*NODE_ID)
            .ok_or_else(|| anyhow!("Port {} has no node ID", global.id))?;
        let node_id = node_id_str
            .parse::<u32>()
            .map_err(|_| anyhow!("Port {} has invalid node ID: {}", global.id, node_id_str))?;
        let direction = match props.get(*PORT_DIRECTION) {
            Some("in") => PortDirection::Input,
            Some("out") => PortDirection::Output,
            _ => return Err(anyhow!("Port {} has invalid direction", global.id)),
        };
        let channel = props.get(*AUDIO_CHANNEL).unwrap_or("unknown").to_string();

        let links = self
            .links
            .values()
            .filter(|l| {
                (l.input_port == global.id && direction == PortDirection::Input)
                    || (l.output_port == global.id && direction == PortDirection::Output)
            })
            .map(|l| l.id)
            .collect();

        let port = PortInternal {
            id: global.id,
            name: name.clone(),
            node_id,
            direction,
            channel,
            links,
            proxy,
        };

        self.ports.insert(global.id, port);
        debug!("Added port {}: '{}' for node {}", global.id, name, node_id);

        if let Some(node) = self.nodes.get_mut(&node_id) {
            if !node.ports.contains(&global.id) {
                node.ports.push(global.id);
            }
        }
        Ok(())
    }

    pub fn add_link(
        &mut self,
        registry: &Rc<pipewire::registry::Registry>,
        global: &GlobalObject<&DictRef>,
    ) -> Result<()> {
        let props = global
            .props
            .ok_or_else(|| anyhow!("Link {} has no props", global.id))?;
        let proxy = registry
            .bind::<pipewire::link::Link, &DictRef>(global)
            .with_context(|| format!("Failed to bind link {}", global.id))?;

        let parse_u32 = |key: &str| -> Result<u32> {
            let str_val = props
                .get(key)
                .ok_or_else(|| anyhow!("Link {} missing property: {}", global.id, key))?;
            str_val.parse::<u32>().map_err(|e| {
                anyhow!(
                    "Link {} invalid u32 for {}: {} ({})",
                    global.id,
                    key,
                    str_val,
                    e
                )
            })
        };

        let output_port = parse_u32(*LINK_OUTPUT_PORT)?;
        let input_port = parse_u32(*LINK_INPUT_PORT)?;
        let output_node = parse_u32(*LINK_OUTPUT_NODE)?;
        let input_node = parse_u32(*LINK_INPUT_NODE)?;

        let link = LinkInternal {
            id: global.id,
            output_node,
            output_port,
            input_node,
            input_port,
            proxy,
        };

        self.links.insert(global.id, link);
        debug!(
            "Added link {} ({}p:{} -> {}p:{})",
            global.id, output_node, output_port, input_node, input_port
        );

        if let Some(port) = self.ports.get_mut(&output_port) {
            if !port.links.contains(&global.id) {
                port.links.push(global.id);
            }
        }
        if let Some(port) = self.ports.get_mut(&input_port) {
            if !port.links.contains(&global.id) {
                port.links.push(global.id);
            }
        }
        Ok(())
    }

    pub fn create_link(&mut self, output_node_id: u32, input_node_id: u32) -> Result<()> {
        let output_node = self
            .nodes
            .get(&output_node_id)
            .ok_or_else(|| anyhow!("Output node {} not found for create_link", output_node_id))?;
        let input_node = self
            .nodes
            .get(&input_node_id)
            .ok_or_else(|| anyhow!("Input node {} not found for create_link", input_node_id))?;

        let output_ports: Vec<&PortInternal> = output_node
            .ports
            .iter()
            .filter_map(|port_id| self.ports.get(port_id))
            .filter(|p| p.direction == PortDirection::Output)
            .collect();
        if output_ports.is_empty() {
            return Err(anyhow!(
                "Output node {} has no output ports",
                output_node_id
            ));
        }

        let input_ports: Vec<&PortInternal> = input_node
            .ports
            .iter()
            .filter_map(|port_id| self.ports.get(port_id))
            .filter(|p| p.direction == PortDirection::Input)
            .collect();
        if input_ports.is_empty() {
            return Err(anyhow!("Input node {} has no input ports", input_node_id));
        }

        let core = self.core.clone();
        let port_pairs = map_ports(&output_ports, &input_ports);
        if port_pairs.is_empty() {
            return Err(anyhow!(
                "No matching ports found between nodes {} and {}",
                output_node_id,
                input_node_id
            ));
        }

        let mut created_count = 0;
        let mut first_error: Option<anyhow::Error> = None;

        for (output_port_id, input_port_id) in port_pairs {
            if self
                .links
                .values()
                .any(|link| link.output_port == output_port_id && link.input_port == input_port_id)
            {
                debug!(
                    "Link {}p -> {}p already exists, skipping.",
                    output_port_id, input_port_id
                );
                continue;
            }

            let props = properties! {
                *LINK_OUTPUT_NODE => output_node_id.to_string(), *LINK_OUTPUT_PORT => output_port_id.to_string(),
                *LINK_INPUT_NODE => input_node_id.to_string(), *LINK_INPUT_PORT => input_port_id.to_string(),
                *OBJECT_LINGER => "true",
            };

            match core.create_object::<pipewire::link::Link>("link-factory", &props) {
                Ok(_) => {
                    debug!(
                        "Sent command to create link: {}p -> {}p",
                        output_port_id, input_port_id
                    );
                    created_count += 1;
                }
                Err(e) => {
                    let err_msg = format!(
                        "Failed to create link {}p -> {}p: {}",
                        output_port_id, input_port_id, e
                    );
                    error!("{}", err_msg);
                    if first_error.is_none() {
                        first_error = Some(anyhow!(err_msg));
                    }
                }
            }
        }

        if let Some(err) = first_error {
            Err(err.context(format!(
                "Encountered error creating links between {} and {}",
                output_node_id, input_node_id
            )))
        } else if created_count == 0 {
            Err(anyhow!(
                "No new links were created between {} and {} (they might already exist).",
                output_node_id,
                input_node_id
            ))
        } else {
            debug!(
                "Sent commands to create {} links between nodes {} and {}",
                created_count, output_node_id, input_node_id
            );
            Ok(())
        }
    }

    pub fn remove_link(&mut self, output_node_id: u32, input_node_id: u32) -> Result<()> {
        if !self.nodes.contains_key(&output_node_id) {
            return Err(anyhow!(
                "Output node {} not found for remove_link",
                output_node_id
            ));
        }
        if !self.nodes.contains_key(&input_node_id) {
            return Err(anyhow!(
                "Input node {} not found for remove_link",
                input_node_id
            ));
        }

        let core = self.core.clone();

        let links_to_remove_ids: Vec<u32> = self
            .links
            .values()
            .filter(|link| link.output_node == output_node_id && link.input_node == input_node_id)
            .map(|link| link.id)
            .collect();

        if links_to_remove_ids.is_empty() {
            debug!(
                "No links found to remove between nodes {} and {}",
                output_node_id, input_node_id
            );
            return Ok(());
        }

        let mut removed_count = 0;
        let mut first_error: Option<anyhow::Error> = None;

        for link_id in links_to_remove_ids {
            if let Some(link_internal) = self.links.remove(&link_id) {
                if let Some(port) = self.ports.get_mut(&link_internal.output_port) {
                    port.links.retain(|&id| id != link_id);
                }
                if let Some(port) = self.ports.get_mut(&link_internal.input_port) {
                    port.links.retain(|&id| id != link_id);
                }

                match core.destroy_object(link_internal.proxy) {
                    Ok(_) => {
                        debug!("Sent command to destroy link object {}", link_id);
                        removed_count += 1;
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to destroy link object {}: {}", link_id, e);
                        error!("{}", err_msg);
                        if first_error.is_none() {
                            first_error = Some(anyhow!(err_msg));
                        }
                    }
                }
            } else {
                warn!(
                    "Link {} was already removed internally before destroy command.",
                    link_id
                );
            }
        }

        debug!(
            "Attempted to remove {} links between nodes {} and {}",
            removed_count, output_node_id, input_node_id
        );
        if let Some(err) = first_error {
            Err(err)
        } else {
            Ok(())
        }
    }
}

pub fn map_ports<'a>(
    output_ports: &[&'a PortInternal],
    input_ports: &[&'a PortInternal],
) -> Vec<(u32, u32)> {
    if output_ports.is_empty() || input_ports.is_empty() {
        return Vec::new();
    }
    if output_ports.len() == 1 {
        return input_ports
            .iter()
            .map(|in_port| (output_ports[0].id, in_port.id))
            .collect();
    }

    let mut pairs = Vec::new();
    let mut used_input_ports = HashSet::new();

    for out_port in output_ports {
        if let Some(matching_input) = input_ports.iter().find(|in_port| {
            !used_input_ports.contains(&in_port.id)
                && !in_port.channel.is_empty()
                && in_port.channel != "unknown"
                && in_port.channel == out_port.channel
        }) {
            pairs.push((out_port.id, matching_input.id));
            used_input_ports.insert(matching_input.id);
        }
    }

    let min_len = output_ports.len().min(input_ports.len());
    if pairs.len() < min_len {
        warn!("Channel matching incomplete ({} pairs for {}/{} ports), attempting positional fallback.", pairs.len(), output_ports.len(), input_ports.len());
        let mut fallback_pairs = Vec::new();
        let mut current_used_inputs = used_input_ports;
        for (i, out_port) in output_ports.iter().enumerate() {
            if pairs.iter().any(|(out_id, _)| *out_id == out_port.id) {
                continue;
            }
            if let Some(in_port) = input_ports.get(i) {
                if !current_used_inputs.contains(&in_port.id) {
                    fallback_pairs.push((out_port.id, in_port.id));
                    current_used_inputs.insert(in_port.id);
                }
            }
        }
        pairs.extend(fallback_pairs);
    }
    pairs
}
