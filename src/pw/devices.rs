use crate::pw::{
    graph::{AudioGraph, Store},
    volume::VolumeResolver,
};
use anyhow::{anyhow, Context as AnyhowContext, Result};
use libspa::{
    pod::builder::Builder,
    sys::{
        spa_pod_frame, SPA_PARAM_PROFILE_available, SPA_PARAM_PROFILE_description,
        SPA_PARAM_PROFILE_index, SPA_PARAM_PROFILE_name, SPA_PARAM_PROFILE_priority,
        SPA_PARAM_PROFILE_save, SPA_TYPE_OBJECT_ParamProfile,
    },
};
use log::{debug, error};
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

fn get_device_bus(props: &DictRef) -> Option<&str> {
    props.get("device.bus")
}

fn get_device_form_factor(props: &DictRef) -> Option<&str> {
    props.get("device.form-factor")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub device_type: DeviceType,
    pub bus: Option<String>,
    pub form_factor: Option<String>,
    pub nodes: Vec<u32>,
    pub profiles: Vec<Profile>,
    pub current_profile_index: Option<u32>,
    pub volume: f32,
    pub muted: bool,
}

pub struct DeviceInternal {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub device_type: DeviceType,
    pub bus: Option<String>,
    pub form_factor: Option<String>,
    pub nodes: Vec<u32>,
    pub profiles: Vec<Profile>,
    pub current_profile_index: Option<u32>,
    pub proxy: pipewire::device::Device,
    pub listener: Option<pipewire::device::DeviceListener>,
    pub volume: f32,
    pub muted: bool,
    pub output_route_index: Option<i32>,
    pub output_route_device: Option<i32>,
    pub input_route_index: Option<i32>,
    pub input_route_device: Option<i32>,
}

impl DeviceInternal {
    pub fn to_device(&self) -> Device {
        Device {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            device_type: self.device_type,
            bus: self.bus.clone(),
            form_factor: self.form_factor.clone(),
            nodes: self.nodes.clone(),
            profiles: self.profiles.clone(),
            current_profile_index: self.current_profile_index,
            volume: self.volume,
            muted: self.muted,
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

        let mut device = DeviceInternal {
            id: global.id,
            name: name.clone(),
            description,
            device_type,
            bus: None,
            form_factor: None,
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
            volume: 1.0,
            muted: false,
            output_route_index: None,
            output_route_device: None,
            input_route_index: None,
            input_route_device: None,
        };

        self.setup_device_monitoring(&mut device, store_rc, graph_tx);

        self.devices.insert(global.id, device);
        debug!("Added device {}: '{}'", global.id, name);
        Ok(())
    }

    fn handle_device_parameter(
        &mut self,
        device_id: u32,
        param_type: ParamType,
        pod: &Pod,
    ) -> bool {
        match param_type {
            ParamType::Route => self
                .parse_route_volume_data(device_id, pod)
                .unwrap_or(false),
            ParamType::Props => self
                .parse_device_props_volume(device_id, pod)
                .unwrap_or(false),
            ParamType::EnumProfile => self
                .handle_device_profile_list(device_id, pod)
                .unwrap_or(false),
            ParamType::Profile => self
                .handle_device_current_profile(device_id, pod)
                .unwrap_or(false),
            _ => false,
        }
    }

    pub fn setup_device_monitoring(
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
            .param({
                let store_weak = store_weak.clone();
                let graph_tx = graph_tx_clone.clone();
                move |_seq, param_type, _index, _next, pod_opt| {
                    if let Some(pod) = pod_opt {
                        if let Some(store_rc) = store_weak.upgrade() {
                            let updated = match store_rc.try_borrow_mut() {
                                Ok(mut store_borrow) => {
                                    store_borrow.handle_device_parameter(device_id, param_type, pod)
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to borrow store for device {}: {}",
                                        device_id, e
                                    );
                                    return;
                                }
                            };

                            if updated {
                                crate::pw::graph::update_graph(&store_rc, &graph_tx);
                            }
                        }
                    }
                }
            })
            .info({
                let store_weak = store_weak.clone();
                let graph_tx = graph_tx_clone.clone();
                move |info| {
                    if let Some(store_rc) = store_weak.upgrade() {
                        let updated = match store_rc.try_borrow_mut() {
                            Ok(mut store_borrow) => {
                                if let Some(props) = info.props() {
                                    let bus = get_device_bus(props).map(str::to_string);
                                    let form_factor =
                                        get_device_form_factor(props).map(str::to_string);

                                    if let Some(device) = store_borrow.devices.get_mut(&device_id) {
                                        let mut updated = false;

                                        if device.bus != bus {
                                            device.bus = bus;
                                            updated = true;
                                        }

                                        if device.form_factor != form_factor {
                                            device.form_factor = form_factor;
                                            updated = true;
                                        }

                                        updated
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            }
                            Err(e) => {
                                error!(
                                    "Failed to borrow store for device info {}: {}",
                                    device_id, e
                                );
                                false
                            }
                        };

                        if updated {
                            crate::pw::graph::update_graph(&store_rc, &graph_tx);
                        }
                    }
                }
            })
            .register();

        device.listener = Some(listener);

        // Subscribe to volume and profile parameters
        device.proxy.subscribe_params(&[
            ParamType::Route,
            ParamType::Props,
            ParamType::EnumProfile,
            ParamType::Profile,
        ]);

        // Initialize parameter enumeration
        device
            .proxy
            .enum_params(0, Some(ParamType::Route), 0, u32::MAX);
        device
            .proxy
            .enum_params(0, Some(ParamType::Props), 0, u32::MAX);
        device
            .proxy
            .enum_params(0, Some(ParamType::EnumProfile), 0, u32::MAX);
        device.proxy.enum_params(0, Some(ParamType::Profile), 0, 1);
    }

    pub fn parse_route_volume_data(&mut self, device_id: u32, pod: &Pod) -> Result<bool> {
        let device = self
            .devices
            .get_mut(&device_id)
            .ok_or_else(|| anyhow!("Device {} not found", device_id))?;

        if let Ok((_, Value::Object(obj))) = PodDeserializer::deserialize_any_from(pod.as_bytes()) {
            let mut route_direction: Option<u32> = None;
            let mut route_index: Option<i32> = None;
            let mut route_device: Option<i32> = None;

            for prop in &obj.properties {
                match prop.key {
                    libspa::sys::SPA_PARAM_ROUTE_direction => {
                        if let Value::Id(spa_id) = &prop.value {
                            route_direction = Some(spa_id.0);
                        }
                    }
                    libspa::sys::SPA_PARAM_ROUTE_index => {
                        if let Value::Int(index) = prop.value {
                            route_index = Some(index);
                        }
                    }
                    libspa::sys::SPA_PARAM_ROUTE_device => {
                        if let Value::Int(device_num) = prop.value {
                            route_device = Some(device_num);
                        }
                    }
                    _ => {}
                }
            }

            // Cache route info for all routes
            if let (Some(direction), Some(index), Some(device_num)) =
                (route_direction, route_index, route_device)
            {
                if direction == 1 {
                    device.output_route_index = Some(index);
                    device.output_route_device = Some(device_num);
                } else if direction == 0 {
                    device.input_route_index = Some(index);
                    device.input_route_device = Some(device_num);
                }
            }

            let should_process_route = matches!(
                (device.device_type, route_direction),
                (DeviceType::Sink, Some(1)) | (DeviceType::Source, Some(0))
            );

            if !should_process_route {
                return Ok(false);
            }

            let mut volume_updated = false;
            let mut mute_updated = false;
            let mut channel_volumes: Option<f32> = None;

            for prop in &obj.properties {
                match prop.key {
                    libspa::sys::SPA_PARAM_ROUTE_props => {
                        if let Value::Object(props_obj) = &prop.value {
                            for volume_prop in &props_obj.properties {
                                match volume_prop.key {
                                    k if k == libspa::sys::SPA_PROP_channelVolumes => {
                                        channel_volumes = VolumeResolver::extract_channel_volume(
                                            &volume_prop.value,
                                        );
                                    }
                                    k if k == libspa::sys::SPA_PROP_mute => {
                                        if let Value::Bool(mute) = volume_prop.value {
                                            if device.muted != mute {
                                                device.muted = mute;
                                                mute_updated = true;
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    k if k == libspa::sys::SPA_PROP_mute => {
                        if let Value::Bool(mute) = prop.value {
                            if device.muted != mute {
                                device.muted = mute;
                                mute_updated = true;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if let Some(ch_vol) = channel_volumes {
                let user_facing_volume = VolumeResolver::apply_cubic_scaling(ch_vol);
                if (device.volume - user_facing_volume).abs() > 0.001 {
                    device.volume = user_facing_volume;
                    volume_updated = true;
                }
            }

            if volume_updated || mute_updated {
                let volume = device.volume;
                let muted = device.muted;
                self.update_node_volumes_from_device(device_id, volume, muted);
            }

            return Ok(volume_updated || mute_updated);
        }

        Ok(false)
    }

    pub fn parse_device_props_volume(&mut self, device_id: u32, pod: &Pod) -> Result<bool> {
        let device = self
            .devices
            .get_mut(&device_id)
            .ok_or_else(|| anyhow!("Device {} not found", device_id))?;

        if let Ok((_, Value::Object(obj))) = PodDeserializer::deserialize_any_from(pod.as_bytes()) {
            let mut updated = false;

            for prop in &obj.properties {
                match prop.key {
                    libspa::sys::SPA_PROP_volume => {
                        if let Value::Float(volume) = prop.value {
                            if (device.volume - volume).abs() > 0.001 {
                                device.volume = volume;
                                updated = true;
                            }
                        }
                    }
                    libspa::sys::SPA_PROP_mute => {
                        if let Value::Bool(mute) = prop.value {
                            if device.muted != mute {
                                device.muted = mute;
                                updated = true;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if updated {
                let device_volume = device.volume;
                let device_muted = device.muted;
                self.update_node_volumes_from_device(device_id, device_volume, device_muted);
            }

            return Ok(updated);
        }

        Ok(false)
    }

    fn update_node_volumes_from_device(&mut self, device_id: u32, volume: f32, muted: bool) {
        let device = match self.devices.get(&device_id) {
            Some(d) => d,
            None => return,
        };

        for &node_id in &device.nodes {
            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.volume = volume;
                node.muted = muted;
            }
        }
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

    fn determine_effective_device_type(&self, device: &DeviceInternal) -> Result<DeviceType> {
        if device.device_type != DeviceType::Unknown {
            return Ok(device.device_type);
        }

        let node_types: Vec<_> = device
            .nodes
            .iter()
            .filter_map(|&node_id| self.nodes.get(&node_id))
            .map(|node| node.node_type)
            .collect();

        if node_types
            .iter()
            .any(|&nt| matches!(nt, crate::pw::nodes::NodeType::Sink))
        {
            Ok(DeviceType::Sink)
        } else if node_types
            .iter()
            .any(|&nt| matches!(nt, crate::pw::nodes::NodeType::Source))
        {
            Ok(DeviceType::Source)
        } else {
            Err(anyhow!(
                "Cannot determine device type for device {}",
                device.id
            ))
        }
    }

    fn get_route_info(
        &self,
        device: &DeviceInternal,
        device_type: DeviceType,
    ) -> Result<(i32, i32)> {
        match device_type {
            DeviceType::Sink => device
                .output_route_index
                .zip(device.output_route_device)
                .ok_or_else(|| anyhow!("No cached output route info for device {}", device.id)),
            DeviceType::Source => device
                .input_route_index
                .zip(device.input_route_device)
                .ok_or_else(|| anyhow!("No cached input route info for device {}", device.id)),
            DeviceType::Unknown => Err(anyhow!("Cannot get route info for Unknown device type")),
        }
    }

    fn build_route_parameter_pod(
        &self,
        route_index: i32,
        route_device: i32,
        props_builder: impl FnOnce(&mut Builder) -> Result<()>,
    ) -> Result<Vec<u8>> {
        let mut buffer: Vec<u8> = Vec::new();
        let mut builder = Builder::new(&mut buffer);
        let mut frame = MaybeUninit::<spa_pod_frame>::uninit();

        unsafe {
            builder
                .push_object(
                    &mut frame,
                    libspa::sys::SPA_TYPE_OBJECT_ParamRoute,
                    ParamType::Route.as_raw(),
                )
                .context("Failed to push Route object")?;
            let initialized_frame = frame.assume_init_mut();

            builder
                .add_prop(libspa::sys::SPA_PARAM_ROUTE_index, 0)
                .and_then(|_| builder.add_int(route_index))
                .and_then(|_| builder.add_prop(libspa::sys::SPA_PARAM_ROUTE_device, 0))
                .and_then(|_| builder.add_int(route_device))
                .and_then(|_| builder.add_prop(libspa::sys::SPA_PARAM_ROUTE_props, 0))
                .context("Failed to add route identification")?;

            let mut props_frame = MaybeUninit::<spa_pod_frame>::uninit();
            builder
                .push_object(
                    &mut props_frame,
                    libspa::sys::SPA_TYPE_OBJECT_Props,
                    libspa::sys::SPA_TYPE_OBJECT_Props,
                )
                .context("Failed to push props object")?;
            let initialized_props_frame = props_frame.assume_init_mut();

            props_builder(&mut builder)?;

            builder.pop(initialized_props_frame);

            builder
                .add_prop(libspa::sys::SPA_PARAM_ROUTE_save, 0)
                .and_then(|_| builder.add_bool(true))
                .context("Failed to add route save")?;

            builder.pop(initialized_frame);
        }

        Ok(buffer)
    }

    pub fn set_device_volume(&mut self, device_id: u32, volume: f32) -> Result<()> {
        let device = self
            .devices
            .get(&device_id)
            .ok_or_else(|| anyhow!("Device {} not found", device_id))?;

        let effective_device_type = self.determine_effective_device_type(device)?;
        let (route_index, route_device) = self.get_route_info(device, effective_device_type)?;

        let raw_volume = VolumeResolver::apply_inverse_cubic_scaling(volume.clamp(0.0, 1.0));

        let buffer = self.build_route_parameter_pod(route_index, route_device, |builder| {
            builder
                .add_prop(libspa::sys::SPA_PROP_channelVolumes, 0)
                .context("Failed to add channelVolumes property")?;

            let volumes = [raw_volume; 2];
            unsafe {
                builder
                    .add_array(
                        std::mem::size_of::<f32>() as u32,
                        pipewire::spa::utils::SpaTypes::Float.as_raw(),
                        volumes.len() as u32,
                        volumes.as_ptr() as *const std::ffi::c_void,
                    )
                    .context("Failed to add volume array")
            }
        })?;

        let pod_ref = Pod::from_bytes(&buffer)
            .ok_or_else(|| anyhow!("Failed to create Pod reference for device volume"))?;

        device.proxy.set_param(ParamType::Route, 0, pod_ref);
        Ok(())
    }

    pub fn set_device_mute(&mut self, device_id: u32, mute: bool) -> Result<()> {
        let device = self
            .devices
            .get(&device_id)
            .ok_or_else(|| anyhow!("Device {} not found", device_id))?;

        let effective_device_type = self.determine_effective_device_type(device)?;
        let (route_index, route_device) = self.get_route_info(device, effective_device_type)?;

        let buffer = self.build_route_parameter_pod(route_index, route_device, |builder| {
            builder
                .add_prop(libspa::sys::SPA_PROP_mute, 0)
                .and_then(|_| builder.add_bool(mute))
                .context("Failed to add mute property")
        })?;

        let pod_ref = Pod::from_bytes(&buffer)
            .ok_or_else(|| anyhow!("Failed to create Pod reference for device mute"))?;

        device.proxy.set_param(ParamType::Route, 0, pod_ref);
        Ok(())
    }
}
