use libspa::pod::{Value, ValueArray};

pub struct VolumeResolver;

impl VolumeResolver {
    pub fn resolve_volume(
        device_volume: Option<f32>,
        device_muted: Option<bool>,
        node_volume: f32,
        node_muted: bool,
    ) -> (f32, bool) {
        if let (Some(dev_vol), Some(dev_muted)) = (device_volume, device_muted) {
            if dev_vol > 0.0 && dev_vol != 1.0 {
                return (dev_vol, dev_muted);
            }
        }

        (node_volume, node_muted)
    }

    pub fn apply_cubic_scaling(raw_volume: f32) -> f32 {
        if raw_volume <= 0.0 {
            0.0
        } else {
            raw_volume.powf(1.0 / 3.0)
        }
    }

    pub fn extract_channel_volume(value: &Value) -> Option<f32> {
        match value {
            Value::ValueArray(ValueArray::Float(float_vec)) => {
                if !float_vec.is_empty() {
                    Some(float_vec[0])
                } else {
                    None
                }
            }
            Value::Float(volume) => Some(*volume),
            _ => None,
        }
    }
}
