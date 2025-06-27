use anyhow::{anyhow, Result};
use log::{debug, info};
use pipewire::metadata::{Metadata, MetadataListener};
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct MetadataManager {
    metadata: Option<Metadata>,
    properties: Rc<RefCell<HashMap<String, String>>>,
    _listener: Option<MetadataListener>,
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
            metadata: None,
            properties: Rc::new(RefCell::new(HashMap::new())),
            _listener: None,
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

    pub fn register_metadata(&mut self, metadata: Metadata) {
        info!("Registered metadata object");

        let properties_clone = self.properties.clone();
        let update_callback = self.update_callback.take();

        let listener = metadata
            .add_listener_local()
            .property(move |subject, key, type_, value| {
                debug!(
                    "Metadata property callback - subject: {subject}, key: {key:?}, type: {type_:?}, value: {value:?}",
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
                    debug!("Cached metadata property: {key_str} = {value_str}");
                } else {
                    properties_clone.borrow_mut().remove(key_str);
                    debug!("Removed metadata property: {key_str}");
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

        self.metadata = Some(metadata);
        self._listener = Some(listener);
        debug!("Metadata listener registered successfully");
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
        self.metadata.is_some()
    }

    fn set_default_audio_device(&self, node_name: &str, device_type: &str) -> Result<()> {
        let metadata = self
            .metadata
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

        debug!("Set default {device_type} to {node_name} in metadata");
        Ok(())
    }

    pub fn set_default_sink(&self, node_name: &str) -> Result<()> {
        self.set_default_audio_device(node_name, "sink")
    }

    pub fn set_default_source(&self, node_name: &str) -> Result<()> {
        self.set_default_audio_device(node_name, "source")
    }
}
