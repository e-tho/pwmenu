use libspa::pod::{Value, ValueArray};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDirection {
    Input,
    Output,
}

pub struct VolumeResolver;

impl VolumeResolver {
    pub fn resolve_effective_volume(
        route_volume: Option<f32>,
        route_muted: Option<bool>,
        node_volume: f32,
        node_muted: bool,
        has_route_volume: bool,
    ) -> (f32, bool) {
        if has_route_volume {
            if let (Some(vol), Some(muted)) = (route_volume, route_muted) {
                return (vol, muted);
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

    pub fn apply_inverse_cubic_scaling(volume: f32) -> f32 {
        if volume <= 0.0 {
            0.0
        } else {
            volume.powf(3.0)
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
