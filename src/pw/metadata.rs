use anyhow::{anyhow, Result};
use log::debug;
use pipewire::metadata::{Metadata, MetadataListener};
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct MetadataManager {
    default_metadata: Option<Metadata>,
    settings_metadata: Option<Metadata>,
    properties: Rc<RefCell<HashMap<String, String>>>,
    settings_properties: Rc<RefCell<HashMap<String, String>>>,
    _default_listener: Option<MetadataListener>,
    _settings_listener: Option<MetadataListener>,
    update_callback: Option<Box<dyn Fn()>>,
}

impl Default for MetadataManager {
    fn default() -> Self {
        Self::new()
    }
}

const GLOBAL_SUBJECT_ID: u32 = 0;
const SPA_JSON_TYPE: &str = "Spa:String:JSON";
const DEFAULT_AUDIO_PREFIX: &str = "default.audio.";
const DEFAULT_CONFIGURED_AUDIO_PREFIX: &str = "default.configured.audio.";

fn is_default_audio_key(key: &str) -> bool {
    key.starts_with(DEFAULT_AUDIO_PREFIX) || key.starts_with(DEFAULT_CONFIGURED_AUDIO_PREFIX)
}

impl MetadataManager {
    pub fn new() -> Self {
        Self {
            default_metadata: None,
            settings_metadata: None,
            properties: Rc::new(RefCell::new(HashMap::new())),
            settings_properties: Rc::new(RefCell::new(HashMap::new())),
            _default_listener: None,
            _settings_listener: None,
            update_callback: None,
        }
    }

    pub fn with_update_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn() + 'static,
    {
        self.update_callback = Some(Box::new(callback));
        self
    }

    pub fn register_default_metadata(&mut self, metadata: Metadata) {
        debug!("Registered default metadata object");

        let properties_clone = self.properties.clone();
        let update_callback = self.update_callback.take();

        let listener = metadata
            .add_listener_local()
            .property(move |subject, key, type_, value| {
                debug!(
                    "Default metadata property callback - subject: {subject}, key: {key:?}, type: {type_:?}, value: {value:?}",
                );

                if subject != GLOBAL_SUBJECT_ID {
                    return 0;
                }

                let Some(key_str) = key else {
                    return 0;
                };

                // Update properties cache
                if let Some(value_str) = value {
                    properties_clone
                        .borrow_mut()
                        .insert(key_str.to_string(), value_str.to_string());
                    debug!("Cached default metadata property: {key_str} = {value_str}");
                } else {
                    properties_clone.borrow_mut().remove(key_str);
                    debug!("Removed default metadata property: {key_str}");
                }

                // Trigger graph update for default audio device changes
                if is_default_audio_key(key_str) {
                    if let Some(ref callback) = update_callback {
                        callback();
                    }
                }

                0
            })
            .register();

        self.default_metadata = Some(metadata);
        self._default_listener = Some(listener);
        debug!("Default metadata listener registered successfully");
    }

    pub fn register_settings_metadata(&mut self, metadata: Metadata) {
        let properties_clone = self.settings_properties.clone();
        let update_callback = self.update_callback.take();

        let listener = metadata
            .add_listener_local()
            .property(move |subject, key, _type_, value| {
                if subject != GLOBAL_SUBJECT_ID {
                    return 0;
                }

                let Some(key_str) = key else {
                    return 0;
                };

                if let Some(value_str) = value {
                    properties_clone
                        .borrow_mut()
                        .insert(key_str.to_string(), value_str.to_string());
                } else {
                    properties_clone.borrow_mut().remove(key_str);
                }

                if key_str == "clock.rate" {
                    if let Some(ref callback) = update_callback {
                        callback();
                    }
                }

                0
            })
            .register();

        self.settings_metadata = Some(metadata);
        self._settings_listener = Some(listener);
    }

    fn get_device_name_from_metadata(&self, key: &str) -> Option<String> {
        self.properties.borrow().get(key).and_then(|json_str| {
            serde_json::from_str::<Value>(json_str)
                .ok()?
                .get("name")?
                .as_str()
                .map(String::from)
        })
    }

    pub fn get_default_sink(&self) -> Option<String> {
        self.get_device_name_from_metadata("default.audio.sink")
            .or_else(|| self.get_device_name_from_metadata("default.configured.audio.sink"))
    }

    pub fn get_default_source(&self) -> Option<String> {
        self.get_device_name_from_metadata("default.audio.source")
            .or_else(|| self.get_device_name_from_metadata("default.configured.audio.source"))
    }

    pub fn is_available(&self) -> bool {
        self.default_metadata.is_some()
    }

    pub fn is_settings_available(&self) -> bool {
        self.settings_metadata.is_some()
    }

    fn set_default_audio_device(&self, node_name: &str, device_type: &str) -> Result<()> {
        let metadata = self
            .default_metadata
            .as_ref()
            .ok_or_else(|| anyhow!("Default metadata object not found"))?;

        let value = format!(r#"{{ "name": "{node_name}" }}"#);
        let property_key = format!("default.audio.{device_type}");
        let configured_key = format!("default.configured.audio.{device_type}");

        // Set current default and persist setting for restart restoration
        metadata.set_property(
            GLOBAL_SUBJECT_ID,
            &property_key,
            Some(SPA_JSON_TYPE),
            Some(&value),
        );
        metadata.set_property(
            GLOBAL_SUBJECT_ID,
            &configured_key,
            Some(SPA_JSON_TYPE),
            Some(&value),
        );

        debug!("Set default {device_type} to {node_name} in default metadata");
        Ok(())
    }

    pub fn set_default_sink(&self, node_name: &str) -> Result<()> {
        self.set_default_audio_device(node_name, "sink")
    }

    pub fn set_default_source(&self, node_name: &str) -> Result<()> {
        self.set_default_audio_device(node_name, "source")
    }

    pub fn set_sample_rate(&self, sample_rate: u32) -> Result<()> {
        let metadata = self
            .settings_metadata
            .as_ref()
            .ok_or_else(|| anyhow!("Settings metadata object not found"))?;

        // Set the desired rate and enforce it immediately
        metadata.set_property(
            GLOBAL_SUBJECT_ID,
            "clock.rate",
            None,
            Some(&sample_rate.to_string()),
        );

        metadata.set_property(
            GLOBAL_SUBJECT_ID,
            "clock.force-rate",
            None,
            Some(&sample_rate.to_string()),
        );

        debug!(
            "Set global clock.rate and clock.force-rate to {} Hz in settings metadata",
            sample_rate
        );
        Ok(())
    }

    pub fn get_sample_rate(&self) -> Option<u32> {
        self.settings_properties
            .borrow()
            .get("clock.rate")
            .and_then(|rate_str| rate_str.parse::<u32>().ok())
    }
}
