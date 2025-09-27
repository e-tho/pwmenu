use anyhow::Result;
use tokio::sync::oneshot;

use crate::pw::volume::RouteDirection;

#[derive(Debug)]
pub enum PwCommand {
    SetNodeVolume {
        node_id: u32,
        volume: f32,
        result_sender: oneshot::Sender<Result<()>>,
    },
    SetNodeMute {
        node_id: u32,
        mute: bool,
        result_sender: oneshot::Sender<Result<()>>,
    },
    CreateLink {
        output_node: u32,
        input_node: u32,
        result_sender: oneshot::Sender<Result<()>>,
    },
    RemoveLink {
        output_node: u32,
        input_node: u32,
        result_sender: oneshot::Sender<Result<()>>,
    },
    SetDefaultSink {
        node_id: u32,
        result_sender: oneshot::Sender<Result<()>>,
    },
    SetDefaultSource {
        node_id: u32,
        result_sender: oneshot::Sender<Result<()>>,
    },
    SwitchDeviceProfile {
        device_id: u32,
        profile_index: u32,
        result_sender: oneshot::Sender<Result<()>>,
    },
    SwitchDeviceProfileWithRestoration {
        device_id: u32,
        profile_index: u32,
        result_sender: oneshot::Sender<Result<()>>,
    },
    SetDeviceVolume {
        device_id: u32,
        volume: f32,
        direction: Option<RouteDirection>,
        result_sender: oneshot::Sender<Result<()>>,
    },
    SetDeviceMute {
        device_id: u32,
        mute: bool,
        direction: Option<RouteDirection>,
        result_sender: oneshot::Sender<Result<()>>,
    },
    SetSampleRate {
        sample_rate: u32,
        result_sender: oneshot::Sender<Result<()>>,
    },
    Exit,
}
