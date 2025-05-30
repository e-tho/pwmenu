use anyhow::{anyhow, Context as AnyhowContext, Result};
use log::debug;
use pipewire::{keys::*, registry::GlobalObject, spa::utils::dict::DictRef};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

use crate::pw::graph::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    Sink,   // Output device
    Source, // Input device
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub device_type: DeviceType,
    pub nodes: Vec<u32>,
}

pub struct DeviceInternal {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub device_type: DeviceType,
    pub nodes: Vec<u32>,
    pub proxy: pipewire::device::Device,
}

impl DeviceInternal {
    pub fn to_device(&self) -> Device {
        Device {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            device_type: self.device_type,
            nodes: self.nodes.clone(),
        }
    }
}

impl Store {
    pub fn add_device(
        &mut self,
        registry: &Rc<pipewire::registry::Registry>,
        global: &GlobalObject<&DictRef>,
    ) -> Result<()> {
        let props = global
            .props
            .ok_or_else(|| anyhow!("Device {} has no props", global.id))?;
        let proxy = registry
            .bind::<pipewire::device::Device, &DictRef>(global)
            .with_context(|| format!("Failed to bind device {}", global.id))?;

        let name = props
            .get(*DEVICE_NAME)
            .or_else(|| props.get(*DEVICE_NICK))
            .or_else(|| props.get(*DEVICE_DESCRIPTION))
            .unwrap_or("Unknown Device")
            .to_string();
        let description = props.get(*DEVICE_DESCRIPTION).map(str::to_string);
        let device_type = match props.get(*MEDIA_CLASS) {
            Some("Audio/Device/Sink") | Some("Audio/Sink") => DeviceType::Sink,
            Some("Audio/Device/Source") | Some("Audio/Source") => DeviceType::Source,
            _ => DeviceType::Unknown,
        };

        let device = DeviceInternal {
            id: global.id,
            name: name.clone(),
            description,
            device_type,
            nodes: self
                .nodes
                .values()
                .filter(|n| n.device_id == Some(global.id))
                .map(|n| n.id)
                .collect(),
            proxy,
        };

        self.devices.insert(global.id, device);
        debug!("Added device {}: '{}'", global.id, name);
        Ok(())
    }

    pub fn get_output_devices(&self) -> Vec<(u32, String)> {
        self.devices
            .values()
            .filter(|d| d.device_type == DeviceType::Sink)
            .map(|d| (d.id, d.name.clone()))
            .collect()
    }

    pub fn get_input_devices(&self) -> Vec<(u32, String)> {
        self.devices
            .values()
            .filter(|d| d.device_type == DeviceType::Source)
            .map(|d| (d.id, d.name.clone()))
            .collect()
    }
}
