use crate::pw::{
    graph::{AudioGraph, Store},
    volume::{RouteDirection, VolumeResolver},
    NodeType,
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
use log::{debug, error, warn};
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteInfo {
    pub index: Option<i32>,
    pub device: Option<i32>,
    pub volume: Option<f32>,
    pub muted: Option<bool>,
}

impl RouteInfo {
    pub fn is_available(&self) -> bool {
        self.index.is_some() && self.device.is_some()
    }

    pub fn get_route_params(&self) -> Option<(i32, i32)> {
        self.index.zip(self.device)
    }

    pub fn get_volume_state(&self) -> Option<(f32, bool)> {
        self.volume.zip(self.muted)
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
    pub nick: Option<String>,
    pub description: Option<String>,
    pub device_type: DeviceType,
    pub bus: Option<String>,
    pub form_factor: Option<String>,
    pub nodes: Vec<u32>,
    pub profiles: Vec<Profile>,
    pub current_profile_index: Option<u32>,
    pub has_route_volume: bool,
    pub output_route: RouteInfo,
    pub input_route: RouteInfo,
}

pub struct DeviceInternal {
    pub id: u32,
    pub name: String,
    pub nick: Option<String>,
    pub description: Option<String>,
    pub device_type: DeviceType,
    pub bus: Option<String>,
    pub form_factor: Option<String>,
    pub nodes: Vec<u32>,
    pub profiles: Vec<Profile>,
    pub current_profile_index: Option<u32>,
    pub proxy: pipewire::device::Device,
    pub listener: Option<pipewire::device::DeviceListener>,
    pub output_route: RouteInfo,
    pub input_route: RouteInfo,
    pub has_route_volume: bool,
    pub output_channel_count: usize,
    pub input_channel_count: usize,
}

impl DeviceInternal {
    pub fn to_device(&self) -> Device {
        Device {
            id: self.id,
            name: self.name.clone(),
            nick: self.nick.clone(),
            description: self.description.clone(),
            device_type: self.device_type,
            bus: self.bus.clone(),
            form_factor: self.form_factor.clone(),
            nodes: self.nodes.clone(),
            profiles: self.profiles.clone(),
            current_profile_index: self.current_profile_index,
            has_route_volume: self.has_route_volume,
            output_route: self.output_route.clone(),
            input_route: self.input_route.clone(),
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

    pub fn get_route_volume(&self, direction: RouteDirection) -> Option<(f32, bool)> {
        match direction {
            RouteDirection::Output => self.output_route.get_volume_state(),
            RouteDirection::Input => self.input_route.get_volume_state(),
        }
    }

    pub fn switch_profile(&self, profile_index: u32) -> Result<()> {
        let target_profile = self.profiles.iter().find(|p| p.index == profile_index);
        if let Some(profile) = target_profile {
            debug!(
                "Switching device {} from profile {} to profile {profile_index}: '{}' ({})",
                self.id,
                self.current_profile_index.unwrap_or(999),
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
        registry: &Rc<pipewire::registry::RegistryRc>,
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

        let nick = props.get(*DEVICE_NICK).map(str::to_string);
        let description = props.get(*DEVICE_DESCRIPTION).map(str::to_string);
        let name = props
            .get(*DEVICE_NAME)
            .or(nick.as_deref())
            .or(description.as_deref())
            .unwrap_or("Unknown Device")
            .to_string();
        let device_type = match props.get(*MEDIA_CLASS) {
            Some("Audio/Device/Sink") | Some("Audio/Sink") => DeviceType::Sink,
            Some("Audio/Device/Source") | Some("Audio/Source") => DeviceType::Source,
            _ => DeviceType::Unknown,
        };

        let mut device = DeviceInternal {
            id: global.id,
            name,
            nick,
            description: None,
            device_type,
            bus: None,
            form_factor: None,
            nodes: Vec::new(),
            profiles: Vec::new(),
            current_profile_index: None,
            proxy,
            listener: None,
            output_route: RouteInfo::default(),
            input_route: RouteInfo::default(),
            has_route_volume: false,
            output_channel_count: 0,
            input_channel_count: 0,
        };

        self.setup_device_monitoring(&mut device, store_rc, graph_tx);

        self.devices.insert(global.id, device);
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
        let device_id = device.id;
        let store_weak = Rc::downgrade(store_rc);
        let graph_tx_clone = graph_tx.clone();

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
                                Ok(mut store) => {
                                    store.handle_device_parameter(device_id, param_type, pod)
                                }
                                Err(_) => false,
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
                            Ok(mut store) => {
                                if let Some(props) = info.props() {
                                    if let Some(device) = store.devices.get_mut(&device_id) {
                                        let mut updated = false;

                                        if let Some(bus) = get_device_bus(props).map(str::to_string)
                                        {
                                            if device.bus.as_ref() != Some(&bus) {
                                                device.bus = Some(bus);
                                                updated = true;
                                            }
                                        }

                                        if let Some(form_factor) =
                                            get_device_form_factor(props).map(str::to_string)
                                        {
                                            if device.form_factor.as_ref() != Some(&form_factor) {
                                                device.form_factor = Some(form_factor);
                                                updated = true;
                                            }
                                        }

                                        if let Some(description) =
                                            props.get("device.description").map(str::to_string)
                                        {
                                            if device.description.as_ref() != Some(&description) {
                                                device.description = Some(description);
                                                updated = true;
                                            }
                                        }

                                        if let Some(nick) =
                                            props.get("device.nick").map(str::to_string)
                                        {
                                            if device.nick.as_ref() != Some(&nick) {
                                                device.nick = Some(nick);
                                                updated = true;
                                            }
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
                                error!("Failed to borrow store for device info {device_id}: {e}");
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

        device.proxy.subscribe_params(&[
            ParamType::Route,
            ParamType::EnumProfile,
            ParamType::Profile,
        ]);

        device
            .proxy
            .enum_params(0, Some(ParamType::Route), 0, u32::MAX);
    }

    pub fn parse_route_volume_data(&mut self, device_id: u32, pod: &Pod) -> Result<bool> {
        let device = self
            .devices
            .get_mut(&device_id)
            .ok_or_else(|| anyhow!("Device {device_id} not found"))?;

        if let Ok((_, Value::Object(obj))) = PodDeserializer::deserialize_any_from(pod.as_bytes()) {
            let mut route_direction: Option<u32> = None;
            let mut route_index: Option<i32> = None;
            let mut route_device: Option<i32> = None;
            let mut has_volume_props = false;
            let mut route_volume: Option<f32> = None;
            let mut route_muted: Option<bool> = None;
            let mut route_channel_count: Option<usize> = None;

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
                    libspa::sys::SPA_PARAM_ROUTE_props => {
                        if let Value::Object(props_obj) = &prop.value {
                            for volume_prop in &props_obj.properties {
                                match volume_prop.key {
                                    k if k == libspa::sys::SPA_PROP_channelVolumes => {
                                        has_volume_props = true;
                                        if let Value::ValueArray(libspa::pod::ValueArray::Float(
                                            ref float_vec,
                                        )) = volume_prop.value
                                        {
                                            route_channel_count = Some(float_vec.len());
                                            if let Some(raw_volume) =
                                                VolumeResolver::extract_channel_volume(
                                                    &volume_prop.value,
                                                )
                                            {
                                                route_volume = Some(
                                                    VolumeResolver::apply_cubic_scaling(raw_volume),
                                                );
                                            }
                                        }
                                    }
                                    k if k == libspa::sys::SPA_PROP_volume => {
                                        has_volume_props = true;
                                        if let Value::Float(vol) = volume_prop.value {
                                            route_volume = Some(vol);
                                        }
                                    }
                                    k if k == libspa::sys::SPA_PROP_mute => {
                                        has_volume_props = true;
                                        if let Value::Bool(mute) = volume_prop.value {
                                            route_muted = Some(mute);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            if let (Some(direction), Some(index), Some(device_num)) =
                (route_direction, route_index, route_device)
            {
                let mut cache_updated = false;

                if direction == 1 {
                    device.output_route.index = Some(index);
                    device.output_route.device = Some(device_num);

                    if let Some(volume) = route_volume {
                        if device.output_route.volume != Some(volume) {
                            device.output_route.volume = Some(volume);
                            cache_updated = true;
                        }
                    }
                    if let Some(muted) = route_muted {
                        if device.output_route.muted != Some(muted) {
                            device.output_route.muted = Some(muted);
                            cache_updated = true;
                        }
                    }
                    if let Some(count) = route_channel_count {
                        if device.output_channel_count != count {
                            device.output_channel_count = count;
                            cache_updated = true;
                        }
                    }
                } else if direction == 0 {
                    device.input_route.index = Some(index);
                    device.input_route.device = Some(device_num);

                    if let Some(volume) = route_volume {
                        if device.input_route.volume != Some(volume) {
                            device.input_route.volume = Some(volume);
                            cache_updated = true;
                        }
                    }
                    if let Some(muted) = route_muted {
                        if device.input_route.muted != Some(muted) {
                            device.input_route.muted = Some(muted);
                            cache_updated = true;
                        }
                    }
                    if let Some(count) = route_channel_count {
                        if device.input_channel_count != count {
                            device.input_channel_count = count;
                            cache_updated = true;
                        }
                    }
                }

                if has_volume_props {
                    device.has_route_volume = true;
                }

                return Ok(cache_updated || route_direction.is_some());
            }
        }

        Ok(false)
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
            .ok_or_else(|| anyhow!("Device {device_id} not found for profile list update"))?;

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
            .ok_or_else(|| anyhow!("Device {device_id} not found for current profile update"))?;

        let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes())
            .map_err(|e| anyhow!("Failed to deserialize current profile pod: {e:?}"))?;

        let Value::Object(obj) = value else {
            return Err(anyhow!("Expected Object value, got {value:?}"));
        };

        for prop in &obj.properties {
            #[allow(non_upper_case_globals)]
            if prop.key == SPA_PARAM_PROFILE_index {
                if let Value::Int(index) = prop.value {
                    if index < 0 {
                        return Err(anyhow!("Invalid negative profile index: {index}"));
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
                        debug!("Device {device_id} profile unchanged: index {new_index}");
                    }
                }
            }
        }

        Ok(false)
    }

    fn parse_profile_from_pod(pod: &Pod) -> Result<Profile> {
        let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes())
            .map_err(|e| anyhow!("Failed to deserialize profile pod: {e:?}"))?;

        let Value::Object(obj) = value else {
            return Err(anyhow!("Expected Object value, got {value:?}"));
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
                            return Err(anyhow!("Invalid negative profile index: {index}"));
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
                            return Err(anyhow!("Invalid negative profile priority: {priority}"));
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
            .ok_or_else(|| anyhow!("Device {device_id} not found for profile switch"))?;

        device.switch_profile(profile_index)
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

    pub fn set_device_volume(
        &mut self,
        device_id: u32,
        volume: f32,
        direction: Option<RouteDirection>,
    ) -> Result<()> {
        let target_direction = if let Some(dir) = direction {
            let device = self
                .devices
                .get(&device_id)
                .ok_or_else(|| anyhow!("Device {device_id} not found"))?;

            match dir {
                RouteDirection::Output => {
                    if device.output_route.is_available() {
                        Some(dir)
                    } else {
                        None
                    }
                }
                RouteDirection::Input => {
                    if device.input_route.is_available() {
                        Some(dir)
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        };

        if let Some(direction) = target_direction {
            let (route_index, route_device, channel_count) = {
                let device = self
                    .devices
                    .get(&device_id)
                    .ok_or_else(|| anyhow!("Device {device_id} not found"))?;

                let count = match direction {
                    RouteDirection::Output => device.output_channel_count,
                    RouteDirection::Input => device.input_channel_count,
                };

                if count == 0 {
                    return Err(anyhow!(
                        "Channel count not yet known for device {device_id}"
                    ));
                }

                match direction {
                    RouteDirection::Output => (
                        device.output_route.get_route_params().unwrap().0,
                        device.output_route.get_route_params().unwrap().1,
                        count,
                    ),
                    RouteDirection::Input => (
                        device.input_route.get_route_params().unwrap().0,
                        device.input_route.get_route_params().unwrap().1,
                        count,
                    ),
                }
            };

            let raw_volume = VolumeResolver::apply_inverse_cubic_scaling(volume.clamp(0.0, 2.0));
            let volumes: Vec<f32> = vec![raw_volume; channel_count];

            let buffer = self.build_route_parameter_pod(route_index, route_device, |builder| {
                builder
                    .add_prop(libspa::sys::SPA_PROP_channelVolumes, 0)
                    .context("Failed to add channelVolumes property")?;

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

            let device = self
                .devices
                .get_mut(&device_id)
                .ok_or_else(|| anyhow!("Device {device_id} not found"))?;

            device.proxy.set_param(ParamType::Route, 0, pod_ref);

            match direction {
                RouteDirection::Output => {
                    device.output_route.volume = Some(volume);
                }
                RouteDirection::Input => {
                    device.input_route.volume = Some(volume);
                }
            }
        } else {
            let node_ids: Vec<u32> = {
                let device = self
                    .devices
                    .get(&device_id)
                    .ok_or_else(|| anyhow!("Device {device_id} not found"))?;
                device.nodes.clone()
            };

            for node_id in node_ids {
                if let Err(e) = self.set_node_volume(node_id, volume) {
                    warn!("Failed to set volume on node {node_id}: {e}");
                }
            }
        }

        Ok(())
    }

    pub fn set_device_mute(
        &mut self,
        device_id: u32,
        mute: bool,
        direction: Option<RouteDirection>,
    ) -> Result<()> {
        let target_direction = if let Some(dir) = direction {
            let device = self
                .devices
                .get(&device_id)
                .ok_or_else(|| anyhow!("Device {device_id} not found"))?;

            match dir {
                RouteDirection::Output => {
                    if device.output_route.is_available() {
                        Some(dir)
                    } else {
                        None
                    }
                }
                RouteDirection::Input => {
                    if device.input_route.is_available() {
                        Some(dir)
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        };

        if let Some(direction) = target_direction {
            let (route_index, route_device) = {
                let device = self
                    .devices
                    .get(&device_id)
                    .ok_or_else(|| anyhow!("Device {device_id} not found"))?;

                match direction {
                    RouteDirection::Output => device.output_route.get_route_params().unwrap(),
                    RouteDirection::Input => device.input_route.get_route_params().unwrap(),
                }
            };

            let buffer = self.build_route_parameter_pod(route_index, route_device, |builder| {
                builder
                    .add_prop(libspa::sys::SPA_PROP_mute, 0)
                    .and_then(|_| builder.add_bool(mute))
                    .context("Failed to add mute property")
            })?;

            let pod_ref = Pod::from_bytes(&buffer)
                .ok_or_else(|| anyhow!("Failed to create Pod reference for device mute"))?;

            let device = self
                .devices
                .get_mut(&device_id)
                .ok_or_else(|| anyhow!("Device {device_id} not found"))?;

            device.proxy.set_param(ParamType::Route, 0, pod_ref);

            match direction {
                RouteDirection::Output => {
                    device.output_route.muted = Some(mute);
                }
                RouteDirection::Input => {
                    device.input_route.muted = Some(mute);
                }
            }
        } else {
            let node_ids: Vec<u32> = {
                let device = self
                    .devices
                    .get(&device_id)
                    .ok_or_else(|| anyhow!("Device {device_id} not found"))?;
                device.nodes.clone()
            };

            for node_id in node_ids {
                if let Err(e) = self.set_node_mute(node_id, mute) {
                    warn!("Failed to set mute on node {node_id}: {e}");
                }
            }
        }

        Ok(())
    }

    pub fn update_device_type_from_nodes(&mut self, device_id: u32) {
        let node_types: Vec<NodeType> = self
            .nodes
            .values()
            .filter(|n| n.device_id == Some(device_id))
            .map(|n| n.node_type)
            .collect();

        if let Some(device) = self.devices.get_mut(&device_id) {
            if device.device_type == DeviceType::Unknown {
                if node_types
                    .iter()
                    .any(|&nt| matches!(nt, NodeType::AudioSink))
                {
                    device.device_type = DeviceType::Sink;
                } else if node_types
                    .iter()
                    .any(|&nt| matches!(nt, NodeType::AudioSource))
                {
                    device.device_type = DeviceType::Source;
                }
            }
        }
    }
}
