use anyhow::{anyhow, Context as AnyhowContext, Result};
use log::{debug, error, info};
use pipewire::{
    core::Info as CoreInfo, main_loop::MainLoop, registry::GlobalObject, spa::utils::dict::DictRef,
    types::ObjectType,
};
use std::{cell::RefCell, rc::Rc};
use tokio::sync::{mpsc, oneshot, watch};

use crate::pw::{
    commands::PwCommand,
    graph::{update_graph, AudioGraph, ConnectionStatus, Store},
};

pub struct PwEngine {
    cmd_tx: mpsc::UnboundedSender<PwCommand>,
    graph_rx: watch::Receiver<AudioGraph>,
    _join_handle: Option<tokio::task::JoinHandle<()>>,
}

impl PwEngine {
    pub async fn new() -> Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<PwCommand>();
        let (graph_tx, graph_rx) = watch::channel(AudioGraph::default());

        let join_handle = tokio::task::spawn_blocking(move || {
            info!("PipeWire blocking thread started.");
            if let Err(e) = run_pipewire_loop(cmd_rx, graph_tx) {
                error!("PipeWire loop exited with error: {:?}", e);
            } else {
                info!("PipeWire loop exited cleanly.");
            }
        });

        Ok(Self {
            cmd_tx,
            graph_rx,
            _join_handle: Some(join_handle),
        })
    }

    pub fn graph(&self) -> AudioGraph {
        self.graph_rx.borrow().clone()
    }

    async fn send_command_and_wait<F, T>(&self, command_builder: F) -> Result<T>
    where
        F: FnOnce(oneshot::Sender<Result<T>>) -> PwCommand,
        T: Send + 'static,
    {
        let (result_tx, result_rx) = oneshot::channel::<Result<T>>();
        let command = command_builder(result_tx);

        self.cmd_tx
            .send(command)
            .map_err(|e| anyhow!("PipeWire thread command channel closed: {}", e))?;

        result_rx
            .await
            .map_err(|e| anyhow!("PipeWire thread result channel closed: {}", e))?
            .context("PipeWire command execution failed")
    }

    pub async fn set_node_volume(&self, node_id: u32, volume: f32) -> Result<()> {
        self.send_command_and_wait(|rs| PwCommand::SetNodeVolume {
            node_id,
            volume,
            result_sender: rs,
        })
        .await
    }

    pub async fn set_node_mute(&self, node_id: u32, mute: bool) -> Result<()> {
        self.send_command_and_wait(|rs| PwCommand::SetNodeMute {
            node_id,
            mute,
            result_sender: rs,
        })
        .await
    }

    pub async fn create_link(&self, output_node: u32, input_node: u32) -> Result<()> {
        self.send_command_and_wait(|rs| PwCommand::CreateLink {
            output_node,
            input_node,
            result_sender: rs,
        })
        .await
    }

    pub async fn remove_link(&self, output_node: u32, input_node: u32) -> Result<()> {
        self.send_command_and_wait(|rs| PwCommand::RemoveLink {
            output_node,
            input_node,
            result_sender: rs,
        })
        .await
    }

    pub async fn set_default_sink(&self, node_id: u32) -> Result<()> {
        self.send_command_and_wait(|rs| PwCommand::SetDefaultSink {
            node_id,
            result_sender: rs,
        })
        .await
    }

    pub async fn set_default_source(&self, node_id: u32) -> Result<()> {
        self.send_command_and_wait(|rs| PwCommand::SetDefaultSource {
            node_id,
            result_sender: rs,
        })
        .await
    }

    pub async fn switch_device_profile(&self, device_id: u32, profile_index: u32) -> Result<()> {
        self.send_command_and_wait(|rs| PwCommand::SwitchDeviceProfile {
            device_id,
            profile_index,
            result_sender: rs,
        })
        .await
    }

    pub async fn switch_device_profile_with_restoration(
        &self,
        device_id: u32,
        profile_index: u32,
    ) -> Result<()> {
        self.send_command_and_wait(|rs| PwCommand::SwitchDeviceProfileWithRestoration {
            device_id,
            profile_index,
            result_sender: rs,
        })
        .await
    }

    pub async fn set_device_volume(&self, device_id: u32, volume: f32) -> Result<()> {
        self.send_command_and_wait(|rs| PwCommand::SetDeviceVolume {
            device_id,
            volume,
            result_sender: rs,
        })
        .await
    }

    pub async fn set_device_mute(&self, device_id: u32, mute: bool) -> Result<()> {
        self.send_command_and_wait(|rs| PwCommand::SetDeviceMute {
            device_id,
            mute,
            result_sender: rs,
        })
        .await
    }
}

impl Drop for PwEngine {
    fn drop(&mut self) {
        info!("PwEngine dropping. Sending Exit command.");
        let _ = self.cmd_tx.send(PwCommand::Exit);
    }
}

fn run_pipewire_loop(
    mut cmd_rx: mpsc::UnboundedReceiver<PwCommand>,
    graph_tx: watch::Sender<AudioGraph>,
) -> Result<()> {
    pipewire::init();
    debug!("PipeWire library initialized.");

    let mainloop = MainLoop::new(None).context("Failed to create PipeWire MainLoop")?;
    let context =
        pipewire::context::Context::new(&mainloop).context("Failed to create PipeWire Context")?;
    let core = Rc::new(
        context
            .connect(Some(pipewire::properties::properties! {
                *pipewire::keys::APP_NAME => "pwmenu",
                *pipewire::keys::APP_ID => "io.github.e-tho.pwmenu"
            }))
            .context("Failed to connect PipeWire Core")?,
    );
    let registry = Rc::new(
        core.get_registry()
            .context("Failed to get PipeWire Registry")?,
    );
    let store = Rc::new(RefCell::new(Store::new(core.clone())));

    // Setup metadata manager with graph update callback
    store.borrow_mut().setup_metadata_manager(&store, &graph_tx);

    // Update the metadata binding section
    let _registry_listener = {
        let store_clone = store.clone();
        let graph_tx_clone = graph_tx.clone();
        let registry_clone = registry.clone();

        registry
            .add_listener_local()
            .global({
                let store_rc = store_clone.clone();
                let registry = registry_clone.clone();
                let graph_tx = graph_tx_clone.clone();

                move |global| {
                    debug!(
                        "Registry: Global event: id {}, type {:?}",
                        global.id, global.type_
                    );

                    if global.type_ == ObjectType::Metadata {
                        if let Some(props) = &global.props {
                            if let Some("default") = props.get("metadata.name") {
                                match registry.bind::<pipewire::metadata::Metadata, &DictRef>(global) {
                                    Ok(metadata) => {
                                        debug!("Found and bound to default metadata object");
                                        if let Ok(mut store) = store_rc.try_borrow_mut() {
                                            if let Some(mm) = &mut store.metadata_manager {
                                                mm.register_metadata(metadata);
                                                debug!("Registered default metadata object");
                                            } else {
                                                debug!("Metadata manager not initialized in store");
                                            }
                                        } else {
                                            error!("Could not borrow store mutably to register metadata");
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to bind to metadata object: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    let result = match store_rc.try_borrow_mut() {
                        Ok(mut store) => store.add_object(&registry, global, &store_rc, &graph_tx),
                        Err(e) => {
                            error!("Failed to borrow store: {}", e);
                            Ok(false)
                        }
                    };

                    match result {
                        Ok(added) => {
                            if added {
                                update_graph(&store_rc, &graph_tx);
                            }
                        }
                        Err(e) => error!("Error adding object {}: {:?}", global.id, e),
                    }
                }
            })
            .global_remove({
                let store_rc = store_clone.clone();
                let graph_tx = graph_tx_clone.clone();

                move |id| {
                    debug!("Registry: Global remove event: id {}", id);
                    if let Ok(mut store) = store_rc.try_borrow_mut() {
                        store.remove_object(id);
                    }
                    update_graph(&store_rc, &graph_tx);
                }
            })
            .register()
    };

    let _core_listener = {
        let store_clone = store.clone();
        let graph_tx_clone = graph_tx.clone();
        let mainloop_clone_err = mainloop.clone();
        core.add_listener_local()
            .info({
                let store = store_clone.clone();
                move |info: &CoreInfo| {
                    store.borrow_mut().set_pwmenu_client_id(info.id());
                    debug!("Core: Info event received for client ID: {}", info.id());
                }
            })
            .error({
                let store = store_clone.clone();
                let graph_tx = graph_tx_clone.clone();
                move |id, seq, res, message| {
                    error!(
                        "PipeWire Core Error: id {}, seq {}, res {}: {}",
                        id, seq, res, message
                    );
                    store.borrow_mut().connection_status = ConnectionStatus::Error;
                    update_graph(&store, &graph_tx);
                    mainloop_clone_err.quit();
                }
            })
            .done({
                let store = store_clone;
                let graph_tx = graph_tx_clone;
                move |_id, _seq| {
                    debug!("Core: Done event received (initial sync likely complete).");
                    update_graph(&store, &graph_tx);
                }
            })
            .register()
    };

    info!("Starting PipeWire event loop...");
    let mainloop_clone = mainloop.clone();
    let loop_ref = mainloop.loop_();

    loop {
        let timeout = std::time::Duration::from_millis(100);
        match loop_ref.iterate(timeout) {
            res if res < 0 => {
                let err_code = nix::errno::Errno::last_raw();
                error!(
                    "Mainloop iterate error. errno: {} ({})",
                    err_code,
                    nix::errno::Errno::from_raw(err_code)
                );
                store.borrow_mut().connection_status = ConnectionStatus::Error;
                update_graph(&store, &graph_tx);
                mainloop_clone.quit();
                break;
            }
            _ => {}
        }

        match cmd_rx.try_recv() {
            Ok(cmd) => {
                debug!("Received command: {:?}", cmd);

                if matches!(cmd, PwCommand::Exit) {
                    info!("Exit command received. Quitting PipeWire loop.");
                    mainloop_clone.quit();
                    break;
                }

                let cmd_processing_result = match cmd {
                    PwCommand::SetNodeVolume {
                        node_id,
                        volume,
                        result_sender,
                    } => result_sender.send(store.borrow_mut().set_node_volume(node_id, volume)),
                    PwCommand::SetNodeMute {
                        node_id,
                        mute,
                        result_sender,
                    } => result_sender.send(store.borrow_mut().set_node_mute(node_id, mute)),
                    PwCommand::CreateLink {
                        output_node,
                        input_node,
                        result_sender,
                    } => {
                        result_sender.send(store.borrow_mut().create_link(output_node, input_node))
                    }
                    PwCommand::RemoveLink {
                        output_node,
                        input_node,
                        result_sender,
                    } => {
                        result_sender.send(store.borrow_mut().remove_link(output_node, input_node))
                    }
                    PwCommand::SetDefaultSink {
                        node_id,
                        result_sender,
                    } => result_sender.send(store.borrow_mut().set_default_sink(node_id)),
                    PwCommand::SetDefaultSource {
                        node_id,
                        result_sender,
                    } => result_sender.send(store.borrow_mut().set_default_source(node_id)),
                    PwCommand::SwitchDeviceProfile {
                        device_id,
                        profile_index,
                        result_sender,
                    } => result_sender.send(
                        store
                            .borrow_mut()
                            .switch_device_profile(device_id, profile_index),
                    ),
                    PwCommand::SwitchDeviceProfileWithRestoration {
                        device_id,
                        profile_index,
                        result_sender,
                    } => result_sender.send(
                        store
                            .borrow_mut()
                            .switch_device_profile_with_restoration(device_id, profile_index),
                    ),
                    PwCommand::SetDeviceVolume {
                        device_id,
                        volume,
                        result_sender,
                    } => {
                        result_sender.send(store.borrow_mut().set_device_volume(device_id, volume))
                    }
                    PwCommand::SetDeviceMute {
                        device_id,
                        mute,
                        result_sender,
                    } => result_sender.send(store.borrow_mut().set_device_mute(device_id, mute)),
                    PwCommand::Exit => unreachable!("Exit handled above"),
                };

                if cmd_processing_result.is_err() {
                    debug!("Command result receiver dropped.");
                }

                update_graph(&store, &graph_tx);
            }
            Err(mpsc::error::TryRecvError::Empty) => {}
            Err(mpsc::error::TryRecvError::Disconnected) => {
                info!("Command channel closed. Quitting PipeWire loop.");
                mainloop_clone.quit();
                break;
            }
        }
    }

    mainloop.quit();

    // Drop resources in reverse init order
    drop(_registry_listener);
    drop(_core_listener);

    {
        let mut store_mut = store.borrow_mut();
        store_mut.nodes.clear();
        store_mut.devices.clear();
        store_mut.ports.clear();
        store_mut.links.clear();
    }

    drop(store);
    drop(registry);
    drop(core);
    drop(mainloop);

    Ok(())
}

impl Store {
    pub fn add_object(
        &mut self,
        registry: &Rc<pipewire::registry::Registry>,
        global: &GlobalObject<&DictRef>,
        store_rc: &Rc<RefCell<Store>>,
        graph_tx: &watch::Sender<AudioGraph>,
    ) -> Result<bool> {
        match global.type_ {
            ObjectType::Device => {
                self.add_device(registry, global, store_rc, graph_tx)?;
            }
            ObjectType::Node => {
                self.add_node(registry, global, store_rc, graph_tx)?;
            }
            ObjectType::Port => {
                self.add_port(registry, global)?;
            }
            ObjectType::Link => {
                self.add_link(registry, global)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub fn remove_object(&mut self, id: u32) {
        if self.devices.remove(&id).is_some() {
            debug!("Removed device {}", id);
        } else if let Some(node) = self.nodes.remove(&id) {
            debug!("Removed node {}: '{}'", id, node.name);
            if self.default_sink == Some(id) {
                self.default_sink = None;
                debug!("Removed default sink (node was removed)");
            }
            if self.default_source == Some(id) {
                self.default_source = None;
                debug!("Removed default source (node was removed)");
            }
            if let Some(device_id) = node.device_id {
                if let Some(device) = self.devices.get_mut(&device_id) {
                    device.nodes.retain(|&n_id| n_id != id);
                }
            }
        } else if let Some(port) = self.ports.remove(&id) {
            debug!("Removed port {}: '{}'", id, port.name);
            if let Some(node) = self.nodes.get_mut(&port.node_id) {
                node.ports.retain(|&p_id| p_id != id);
            }
            let affected_links = port.links.clone();
            for link_id in affected_links {
                if let Some(link) = self.links.get(&link_id) {
                    let other_port_id = if link.output_port == id {
                        link.input_port
                    } else {
                        link.output_port
                    };
                    if let Some(other_port) = self.ports.get_mut(&other_port_id) {
                        other_port.links.retain(|&l_id| l_id != link_id);
                    }
                }
                if self.links.remove(&link_id).is_some() {
                    debug!("Cascaded removal of link {} due to port removal", link_id);
                }
            }
        } else if let Some(removed_link) = self.links.remove(&id) {
            debug!("Removed link {}", id);
            if let Some(port) = self.ports.get_mut(&removed_link.output_port) {
                port.links.retain(|&l_id| l_id != id);
            }
            if let Some(port) = self.ports.get_mut(&removed_link.input_port) {
                port.links.retain(|&l_id| l_id != id);
            }
        }
    }
}
