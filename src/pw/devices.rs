use crate::pw::graph::{AudioGraph, Store};
use anyhow::{anyhow, Context as AnyhowContext, Result};
use libspa::{
    pod::builder::Builder,
    sys::{
        spa_pod_frame, SPA_PARAM_PROFILE_available, SPA_PARAM_PROFILE_description,
        SPA_PARAM_PROFILE_index, SPA_PARAM_PROFILE_name, SPA_PARAM_PROFILE_priority,
        SPA_PARAM_PROFILE_save, SPA_TYPE_OBJECT_ParamProfile,
    },
};
use log::debug;
use pipewire::spa::{
    param::ParamType,
    pod::{deserialize::PodDeserializer, Pod, Value},
};
use pipewire::{keys::*, registry::GlobalObject, spa::utils::dict::DictRef};
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::{cell::RefCell, mem::MaybeUninit};
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    Sink,   // Output device
    Source, // Input device
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub index: u32,
    pub name: String,
    pub description: String,
    pub priority: u32,
    pub available: String,
}

impl Profile {
    pub fn is_available(&self) -> bool {
        self.available == "yes" || self.available == "unknown"
    }

    pub fn is_off(&self) -> bool {
        self.name == "off"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ConnectionType {
    Usb = 1,
    Internal = 2,
    DisplayAudio = 3,
    Bluetooth = 4,
    Unknown = 5,
}

impl ConnectionType {
    pub fn from_properties(props: &DictRef) -> Self {
        if let Some(bus) = props.get("device.bus") {
            match bus {
                "usb" => return ConnectionType::Usb,
                "bluetooth" => return ConnectionType::Bluetooth,
                "pci" => {
                    if let Some(form_factor) = props.get("device.form_factor") {
                        if form_factor == "hdmi" || form_factor == "displayport" {
                            return ConnectionType::DisplayAudio;
                        }
                    }
                    return ConnectionType::Internal;
                }
                _ => {}
            }
        }

        if let Some(device_name) = props.get("device.name") {
            if device_name.starts_with("alsa_card.usb-") {
                return ConnectionType::Usb;
            }
            if device_name.starts_with("alsa_card.pci-") {
                if let Some(desc) = props.get("device.description") {
                    if desc.to_lowercase().contains("hdmi")
                        || desc.to_lowercase().contains("displayport")
                    {
                        return ConnectionType::DisplayAudio;
                    }
                }
                if let Some(nick) = props.get("device.nick") {
                    if nick.to_lowercase().contains("hdmi") {
                        return ConnectionType::DisplayAudio;
                    }
                }
                return ConnectionType::Internal;
            }
            if device_name.contains("bluez") {
                return ConnectionType::Bluetooth;
            }
        }

        ConnectionType::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub device_type: DeviceType,
    pub connection_type: ConnectionType,
    pub nodes: Vec<u32>,
    pub profiles: Vec<Profile>,
    pub current_profile_index: Option<u32>,
}

pub struct DeviceInternal {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub device_type: DeviceType,
    pub connection_type: ConnectionType,
    pub nodes: Vec<u32>,
    pub profiles: Vec<Profile>,
    pub current_profile_index: Option<u32>,
    pub proxy: pipewire::device::Device,
    pub listener: Option<pipewire::device::DeviceListener>,
}

impl DeviceInternal {
    pub fn to_device(&self) -> Device {
        Device {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            device_type: self.device_type,
            connection_type: self.connection_type,
            nodes: self.nodes.clone(),
            profiles: self.profiles.clone(),
            current_profile_index: self.current_profile_index,
        }
    }

    pub fn get_available_profiles(&self) -> Vec<&Profile> {
        self.profiles
            .iter()
            .filter(|p| p.is_available() && !p.is_off())
            .collect()
    }

    pub fn get_current_profile(&self) -> Option<&Profile> {
        self.current_profile_index
            .and_then(|index| self.profiles.iter().find(|p| p.index == index))
    }

    pub fn switch_profile(&self, profile_index: u32) -> Result<()> {
        let target_profile = self.profiles.iter().find(|p| p.index == profile_index);
        if let Some(profile) = target_profile {
            debug!(
                "Switching device {} from profile {} to profile {}: '{}' ({})",
                self.id,
                self.current_profile_index.unwrap_or(999),
                profile_index,
                profile.name,
                profile.description
            );
        } else {
            return Err(anyhow!(
                "Profile {} not found for device {} (available: {})",
                profile_index,
                self.id,
                self.profiles
                    .iter()
                    .map(|p| p.index.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        let pod_data = self.build_profile_switch_pod(profile_index)?;

        let pod_ref = Pod::from_bytes(&pod_data)
            .ok_or_else(|| anyhow!("Failed to create Pod reference for profile switch"))?;

        self.proxy.set_param(ParamType::Profile, 0, pod_ref);

        self.proxy.enum_params(0, Some(ParamType::Profile), 0, 1);

        debug!(
            "Sent profile switch command for device {} to profile {} via set_param and requested profile update",
            self.id, profile_index
        );
        Ok(())
    }

    fn build_profile_switch_pod(&self, profile_index: u32) -> Result<Vec<u8>> {
        let mut buffer: Vec<u8> = Vec::new();
        let mut builder = Builder::new(&mut buffer);
        let mut frame = MaybeUninit::<spa_pod_frame>::uninit();

        unsafe {
            builder
                .push_object(
                    &mut frame,
                    SPA_TYPE_OBJECT_ParamProfile,
                    ParamType::Profile.as_raw(),
                )
                .context("Failed to push profile object")?;

            let initialized_frame = frame.assume_init_mut();

            builder
                .add_prop(SPA_PARAM_PROFILE_index, 0)
                .context("Failed to add profile index property")?;
            builder
                .add_int(profile_index as i32)
                .context("Failed to add profile index value")?;

            builder
                .add_prop(SPA_PARAM_PROFILE_save, 0)
                .context("Failed to add profile save property")?;
            builder
                .add_bool(true)
                .context("Failed to add profile save value")?;

            builder.pop(initialized_frame);
        }

        Ok(buffer)
    }
}

impl Store {
    pub fn add_device(
        &mut self,
        registry: &Rc<pipewire::registry::Registry>,
        global: &GlobalObject<&DictRef>,
        store_rc: &Rc<RefCell<Store>>,
        graph_tx: &watch::Sender<AudioGraph>,
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
        let connection_type = ConnectionType::from_properties(props);

        let mut device = DeviceInternal {
            id: global.id,
            name: name.clone(),
            description,
            device_type,
            connection_type,
            nodes: self
                .nodes
                .values()
                .filter(|n| n.device_id == Some(global.id))
                .map(|n| n.id)
                .collect(),
            profiles: Vec::new(),
            current_profile_index: None,
            proxy,
            listener: None,
        };

        self.setup_device_profile_monitoring(&mut device, store_rc, graph_tx);

        self.devices.insert(global.id, device);
        debug!(
            "Added device {}: '{}' - requesting profiles",
            global.id, name
        );
        Ok(())
    }

    fn setup_device_profile_monitoring(
        &self,
        device: &mut DeviceInternal,
        store_rc: &Rc<RefCell<Store>>,
        graph_tx: &watch::Sender<AudioGraph>,
    ) {
        let store_weak = Rc::downgrade(store_rc);
        let graph_tx_clone = graph_tx.clone();
        let device_id = device.id;

        let listener = device
            .proxy
            .add_listener_local()
            .param(move |_seq, param_type, _index, _next, pod_opt| {
                if let Some(pod) = pod_opt {
                    if let Some(store_rc) = store_weak.upgrade() {
                        let updated = {
                            let mut store_borrow = match store_rc.try_borrow_mut() {
                                Ok(s) => s,
                                Err(e) => {
                                    log::error!(
                                        "Failed to borrow store in device param callback for device {}: {}",
                                        device_id,
                                        e
                                    );
                                    return;
                                }
                            };

                            match param_type {
                                ParamType::EnumProfile => {
                                    match store_borrow.handle_device_profile_list(device_id, pod) {
                                        Ok(updated) => updated,
                                        Err(e) => {
                                            log::error!(
                                                "Failed to handle profile list for device {}: {}",
                                                device_id, e
                                            );
                                            false
                                        }
                                    }
                                }
                                ParamType::Profile => {
                                    match store_borrow.handle_device_current_profile(device_id, pod) {
                                        Ok(updated) => updated,
                                        Err(e) => {
                                            log::error!(
                                                "Failed to handle current profile for device {}: {}",
                                                device_id, e
                                            );
                                            false
                                        }
                                    }
                                }
                                _ => false,
                            }
                        };

                        if updated {
                            crate::pw::graph::update_graph(&store_rc, &graph_tx_clone);
                        }
                    } else {
                        log::warn!("Store reference expired in device param callback for device {}", device_id);
                    }
                }
            })
            .register();

        device.listener = Some(listener);

        device
            .proxy
            .subscribe_params(&[ParamType::EnumProfile, ParamType::Profile]);

        device
            .proxy
            .enum_params(0, Some(ParamType::EnumProfile), 0, u32::MAX);
        device.proxy.enum_params(0, Some(ParamType::Profile), 0, 1);
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

    pub fn handle_device_profile_list(&mut self, device_id: u32, pod: &Pod) -> Result<bool> {
        // Parse the profile first to avoid borrowing conflicts
        let profile = Self::parse_profile_from_pod(pod)?;

        let device = self
            .devices
            .get_mut(&device_id)
            .ok_or_else(|| anyhow!("Device {} not found for profile list update", device_id))?;

        debug!(
            "Updated profile {} for device {}: '{}' ({}) - available: {}",
            profile.index, device_id, profile.name, profile.description, profile.available
        );

        // Add or update profile
        if let Some(existing) = device
            .profiles
            .iter_mut()
            .find(|p| p.index == profile.index)
        {
            *existing = profile;
        } else {
            device.profiles.push(profile);
        }

        // Sort profiles by priority (descending)
        device.profiles.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(true)
    }

    pub fn handle_device_current_profile(&mut self, device_id: u32, pod: &Pod) -> Result<bool> {
        let device = self
            .devices
            .get_mut(&device_id)
            .ok_or_else(|| anyhow!("Device {} not found for current profile update", device_id))?;

        let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes())
            .map_err(|e| anyhow!("Failed to deserialize current profile pod: {:?}", e))?;

        let Value::Object(obj) = value else {
            return Err(anyhow!("Expected Object value, got {:?}", value));
        };

        for prop in &obj.properties {
            #[allow(non_upper_case_globals)]
            if prop.key == SPA_PARAM_PROFILE_index {
                if let Value::Int(index) = prop.value {
                    if index < 0 {
                        return Err(anyhow!("Invalid negative profile index: {}", index));
                    }

                    let new_index = index as u32;
                    let old_index = device.current_profile_index;

                    if device.current_profile_index != Some(new_index) {
                        device.current_profile_index = Some(new_index);

                        debug!(
                            "Device {} profile changed: {} (index {}) -> {} (index {})",
                            device_id,
                            old_index
                                .and_then(|idx| device.profiles.iter().find(|p| p.index == idx))
                                .map(|p| p.description.as_str())
                                .unwrap_or("Unknown"),
                            old_index.unwrap_or(999),
                            device
                                .profiles
                                .iter()
                                .find(|p| p.index == new_index)
                                .map(|p| p.description.as_str())
                                .unwrap_or("Unknown"),
                            new_index
                        );
                        return Ok(true);
                    } else {
                        debug!(
                            "Device {} profile unchanged: index {}",
                            device_id, new_index
                        );
                    }
                }
            }
        }

        Ok(false)
    }

    fn parse_profile_from_pod(pod: &Pod) -> Result<Profile> {
        let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes())
            .map_err(|e| anyhow!("Failed to deserialize profile pod: {:?}", e))?;

        let Value::Object(obj) = value else {
            return Err(anyhow!("Expected Object value, got {:?}", value));
        };

        let mut profile = Profile {
            index: 0,
            name: String::new(),
            description: String::new(),
            priority: 0,
            available: "unknown".to_string(),
        };

        for prop in &obj.properties {
            #[allow(non_upper_case_globals)]
            match prop.key {
                SPA_PARAM_PROFILE_index => {
                    if let Value::Int(index) = prop.value {
                        if index < 0 {
                            return Err(anyhow!("Invalid negative profile index: {}", index));
                        }
                        profile.index = index as u32;
                    }
                }
                SPA_PARAM_PROFILE_name => {
                    if let Value::String(name) = &prop.value {
                        profile.name = name.clone();
                    }
                }
                SPA_PARAM_PROFILE_description => {
                    if let Value::String(desc) = &prop.value {
                        profile.description = desc.clone();
                    }
                }
                SPA_PARAM_PROFILE_priority => {
                    if let Value::Int(priority) = prop.value {
                        if priority < 0 {
                            return Err(anyhow!("Invalid negative profile priority: {}", priority));
                        }
                        profile.priority = priority as u32;
                    }
                }
                SPA_PARAM_PROFILE_available => {
                    if let Value::String(available) = &prop.value {
                        profile.available = available.clone();
                    }
                }
                _ => {}
            }
        }

        Ok(profile)
    }

    pub fn get_device_profiles(&self, device_id: u32) -> Vec<Profile> {
        self.devices
            .get(&device_id)
            .map(|device| {
                device
                    .profiles
                    .iter()
                    .filter(|p| p.is_available() && !p.is_off())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_device_current_profile(&self, device_id: u32) -> Option<Profile> {
        self.devices
            .get(&device_id)
            .and_then(|device| device.get_current_profile().cloned())
    }

    pub fn switch_device_profile(&mut self, device_id: u32, profile_index: u32) -> Result<()> {
        let device = self
            .devices
            .get(&device_id)
            .ok_or_else(|| anyhow!("Device {} not found for profile switch", device_id))?;

        device.switch_profile(profile_index)
    }
}
