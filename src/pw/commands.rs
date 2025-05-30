use anyhow::Result;
use tokio::sync::oneshot;

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
    Exit,
}
