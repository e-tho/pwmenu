use crate::{
    icons::Icons,
    menu::{
        DeviceMenuOptions, InputDeviceMenuOptions, MainMenuOptions, Menu, OutputDeviceMenuOptions,
        ProfileMenuOptions, SampleRateMenuOptions, SettingsMenuOptions, StreamMenuOptions,
        VolumeMenuOptions,
    },
    notification::NotificationManager,
    pw::{controller::Controller, nodes::Node, Profile},
};
use anyhow::Result;
use log::{debug, info};
use rust_i18n::t;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub struct App {
    pub running: bool,
    pub interactive: bool,
    controller: Controller,
    notification_manager: Arc<NotificationManager>,
    volume_step: f32,
}

impl App {
    pub async fn new(
        _menu: Menu,
        icons: Arc<Icons>,
        volume_step: f32,
        interactive: bool,
    ) -> Result<Self> {
        let controller = Controller::new().await?;
        let notification_manager = Arc::new(NotificationManager::new(icons.clone()));

        info!("{}", t!("notifications.pw.initialized"));

        Ok(Self {
            running: true,
            interactive,
            controller,
            notification_manager,
            volume_step,
        })
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub async fn wait_for_initialization(&self) -> Result<()> {
        self.controller.wait_for_initialization().await
    }

    pub async fn run(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<String>> {
        while self.running {
            match menu.show_main_menu(menu_command, icon_type, spaces).await? {
                Some(main_menu_option) => {
                    self.handle_main_options(
                        menu,
                        menu_command,
                        icon_type,
                        spaces,
                        main_menu_option,
                    )
                    .await?;
                }
                None => {
                    debug!("{}", t!("notifications.pw.main_menu_exited"));
                    self.running = false;
                }
            }
        }

        Ok(None)
    }

    pub async fn run_output_device_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<String>> {
        self.handle_output_device_menu(menu, menu_command, icon_type, spaces)
            .await?;
        Ok(None)
    }

    pub async fn run_input_device_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<String>> {
        self.handle_input_device_menu(menu, menu_command, icon_type, spaces)
            .await?;
        Ok(None)
    }

    pub async fn run_output_streams_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<String>> {
        self.handle_output_streams_menu(menu, menu_command, icon_type, spaces)
            .await?;
        Ok(None)
    }

    pub async fn run_input_streams_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<String>> {
        self.handle_input_streams_menu(menu, menu_command, icon_type, spaces)
            .await?;
        Ok(None)
    }

    async fn handle_main_options(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
        main_menu_option: MainMenuOptions,
    ) -> Result<Option<String>> {
        match main_menu_option {
            MainMenuOptions::ShowOutputDeviceMenu => {
                self.handle_output_device_menu(menu, menu_command, icon_type, spaces)
                    .await?;
            }
            MainMenuOptions::ShowInputDeviceMenu => {
                self.handle_input_device_menu(menu, menu_command, icon_type, spaces)
                    .await?;
            }
            MainMenuOptions::ShowOutputStreamsMenu => {
                self.handle_output_streams_menu(menu, menu_command, icon_type, spaces)
                    .await?;
            }
            MainMenuOptions::ShowInputStreamsMenu => {
                self.handle_input_streams_menu(menu, menu_command, icon_type, spaces)
                    .await?;
            }
            MainMenuOptions::ShowSettingsMenu => {
                self.handle_settings_menu(menu, menu_command, icon_type, spaces)
                    .await?;
            }
        }
        Ok(None)
    }

    async fn handle_settings_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<()> {
        let mut stay_in_settings_menu = true;

        while stay_in_settings_menu {
            let should_stay = self
                .handle_settings_options(menu, menu_command, icon_type, spaces)
                .await?;

            if !should_stay {
                stay_in_settings_menu = false;
            }
        }

        Ok(())
    }

    async fn handle_settings_options(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<bool> {
        let option = menu
            .show_settings_menu(menu_command, icon_type, spaces, self.interactive)
            .await?;

        match option {
            Some(SettingsMenuOptions::SetSampleRate) => {
                self.handle_sample_rate_menu(menu, menu_command, icon_type, spaces)
                    .await?;
                if !self.interactive {
                    self.running = false;
                    return Ok(false);
                }
                Ok(true)
            }
            Some(SettingsMenuOptions::Back) => Ok(false),
            None => {
                if !self.interactive {
                    self.running = false;
                }
                debug!("Exited settings menu");
                Ok(false)
            }
        }
    }

    async fn handle_sample_rate_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<()> {
        let mut stay_in_sample_rate_menu = true;

        while stay_in_sample_rate_menu {
            let should_stay = self
                .handle_sample_rate_options(menu, menu_command, icon_type, spaces)
                .await?;

            if !should_stay {
                stay_in_sample_rate_menu = false;
            }
        }

        Ok(())
    }

    async fn handle_sample_rate_options(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<bool> {
        let current_rate = self.controller.get_system_default_sample_rate();

        let option = menu
            .show_sample_rate_menu(
                menu_command,
                icon_type,
                spaces,
                current_rate,
                self.interactive,
            )
            .await?;

        match option {
            Some(SampleRateMenuOptions::SelectRate(sample_rate)) => {
                self.perform_sample_rate_change(sample_rate).await?;
                if !self.interactive {
                    self.running = false;
                    return Ok(false);
                }
                Ok(true)
            }
            Some(SampleRateMenuOptions::Back) => Ok(false),
            None => {
                if !self.interactive {
                    self.running = false;
                }
                debug!("Exited sample rate menu");
                Ok(false)
            }
        }
    }

    async fn handle_output_streams_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<()> {
        let mut stay_in_streams_menu = true;

        while stay_in_streams_menu {
            let should_stay = self
                .handle_stream_options(menu, menu_command, icon_type, spaces, true)
                .await?;

            if !should_stay {
                stay_in_streams_menu = false;
            }
        }

        Ok(())
    }

    async fn handle_input_streams_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<()> {
        let mut stay_in_streams_menu = true;

        while stay_in_streams_menu {
            let should_stay = self
                .handle_stream_options(menu, menu_command, icon_type, spaces, false)
                .await?;

            if !should_stay {
                stay_in_streams_menu = false;
            }
        }

        Ok(())
    }

    async fn handle_stream_options(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
        is_output: bool,
    ) -> Result<bool> {
        let streams = if is_output {
            self.controller.get_output_streams()
        } else {
            self.controller.get_input_streams()
        };

        let menu_result = menu
            .show_stream_menu(
                menu_command,
                &streams,
                &self.controller,
                icon_type,
                spaces,
                is_output,
                self.interactive,
            )
            .await?;

        match menu_result {
            Some(selection) => {
                if selection == t!("menus.common.back").as_ref() {
                    return Ok(false);
                }

                let refresh_text = StreamMenuOptions::RefreshList.to_str();
                if selection == refresh_text.as_ref() {
                    Ok(true)
                } else {
                    if let Some(stream) = self.find_stream_by_name(&streams, &selection, menu) {
                        self.handle_volume_menu(
                            menu,
                            menu_command,
                            &stream,
                            icon_type,
                            spaces,
                            is_output,
                        )
                        .await?;
                        if !self.running {
                            return Ok(false);
                        }
                        if !self.interactive {
                            self.running = false;
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
            }
            None => {
                if !self.interactive {
                    self.running = false;
                }
                let message = if is_output {
                    t!("notifications.pw.output_streams_menu_exited")
                } else {
                    t!("notifications.pw.input_streams_menu_exited")
                };
                debug!("{message}");
                Ok(false)
            }
        }
    }

    fn find_stream_by_name(&self, streams: &[Node], selection: &str, menu: &Menu) -> Option<Node> {
        let base_selection = if let Some(pos) = selection.find(" [") {
            &selection[..pos]
        } else {
            selection
        };

        for stream in streams {
            let display_name = menu.format_stream_display_name(stream, &self.controller);
            if display_name == base_selection {
                return Some(stream.clone());
            }
        }
        None
    }

    async fn handle_output_device_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<()> {
        let mut stay_in_output_menu = true;

        while stay_in_output_menu {
            let should_stay = self
                .handle_output_device_options(menu, menu_command, icon_type, spaces)
                .await?;

            if !should_stay {
                stay_in_output_menu = false;
            }
        }

        Ok(())
    }

    async fn handle_output_device_options(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<bool> {
        let nodes = self.controller.get_output_nodes();
        let menu_result = menu
            .show_output_device_menu(
                menu_command,
                &nodes,
                &self.controller,
                icon_type,
                spaces,
                self.interactive,
            )
            .await?;

        match menu_result {
            Some(selection) => {
                if selection == t!("menus.common.back").as_ref() {
                    return Ok(false);
                }

                let refresh_text = OutputDeviceMenuOptions::RefreshList.to_str();
                if selection == refresh_text.as_ref() {
                    Ok(true)
                } else {
                    let selected_node =
                        self.handle_device_selection(&nodes, &selection, menu, icon_type, spaces)?;
                    if let Some(node) = selected_node {
                        self.handle_device_menu(menu, menu_command, &node, icon_type, spaces, true)
                            .await?;
                        if !self.running {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
            }
            None => {
                if !self.interactive {
                    self.running = false;
                }
                debug!("{}", t!("notifications.pw.output_devices_menu_exited"));
                Ok(false)
            }
        }
    }

    async fn handle_input_device_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<()> {
        let mut stay_in_input_menu = true;

        while stay_in_input_menu {
            let should_stay = self
                .handle_input_device_options(menu, menu_command, icon_type, spaces)
                .await?;

            if !should_stay {
                stay_in_input_menu = false;
            }
        }

        Ok(())
    }

    async fn handle_input_device_options(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<bool> {
        let nodes = self.controller.get_input_nodes();
        let menu_result = menu
            .show_input_device_menu(
                menu_command,
                &nodes,
                &self.controller,
                icon_type,
                spaces,
                self.interactive,
            )
            .await?;

        match menu_result {
            Some(selection) => {
                if selection == t!("menus.common.back").as_ref() {
                    return Ok(false);
                }

                let refresh_text = InputDeviceMenuOptions::RefreshList.to_str();
                if selection == refresh_text.as_ref() {
                    Ok(true)
                } else {
                    let selected_node =
                        self.handle_device_selection(&nodes, &selection, menu, icon_type, spaces)?;
                    if let Some(node) = selected_node {
                        self.handle_device_menu(
                            menu,
                            menu_command,
                            &node,
                            icon_type,
                            spaces,
                            false,
                        )
                        .await?;
                        if !self.running {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
            }
            None => {
                if !self.interactive {
                    self.running = false;
                }
                debug!("{}", t!("notifications.pw.input_devices_menu_exited"));
                Ok(false)
            }
        }
    }

    fn handle_device_selection(
        &self,
        nodes: &[Node],
        selection: &str,
        menu: &Menu,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<Node>> {
        for node in nodes {
            let formatted = menu.format_node_display(node, &self.controller, icon_type, spaces);
            let cleaned_formatted = menu.clean_menu_output(&formatted, icon_type);

            if cleaned_formatted == selection {
                return Ok(Some(node.clone()));
            }
        }

        Ok(None)
    }

    fn find_replacement_node(&self, original_node: &Node, is_output: bool) -> Option<Node> {
        let device_id = original_node.device_id?;

        let nodes = if is_output {
            self.controller.get_output_nodes()
        } else {
            self.controller.get_input_nodes()
        };

        nodes.into_iter().find(|n| n.device_id == Some(device_id))
    }

    async fn handle_device_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        node: &Node,
        icon_type: &str,
        spaces: usize,
        is_output: bool,
    ) -> Result<()> {
        let mut stay_in_device_menu = true;
        let mut current_node = node.clone();

        while stay_in_device_menu {
            if let Some(updated_node) = self.controller.get_node(current_node.id) {
                if updated_node.node_type == current_node.node_type {
                    current_node = updated_node;
                } else if let Some(replacement) =
                    self.find_replacement_node(&current_node, is_output)
                {
                    debug!(
                        "Device node changed after profile switch, using new node: {}",
                        replacement
                            .description
                            .as_ref()
                            .unwrap_or(&replacement.name)
                    );
                    current_node = replacement;
                } else {
                    return Ok(());
                }
            } else if let Some(replacement) = self.find_replacement_node(&current_node, is_output) {
                current_node = replacement;
            } else {
                return Ok(());
            }

            let should_stay = self
                .handle_device_options(
                    menu,
                    menu_command,
                    &current_node,
                    icon_type,
                    spaces,
                    is_output,
                )
                .await?;

            if !should_stay {
                stay_in_device_menu = false;
            }
        }

        Ok(())
    }

    async fn handle_device_options(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        node: &Node,
        icon_type: &str,
        spaces: usize,
        is_output: bool,
    ) -> Result<bool> {
        let has_profiles = if let Some(device_id) = node.device_id {
            let profiles = self.controller.get_device_profiles(device_id);
            profiles.len() > 1
        } else {
            false
        };

        let device_name = self.controller.get_device_name(node.device_id.unwrap_or(0));

        let option = menu
            .show_device_options(
                menu_command,
                icon_type,
                spaces,
                &device_name,
                node.is_default,
                is_output,
                has_profiles,
                self.interactive,
            )
            .await?;

        match option {
            Some(DeviceMenuOptions::SetDefault) => {
                self.perform_set_default(node, is_output).await?;
                if !self.interactive {
                    self.running = false;
                    return Ok(false);
                }
                Ok(true)
            }
            Some(DeviceMenuOptions::SwitchProfile) => {
                if let Some(device_id) = node.device_id {
                    self.handle_profile_menu(menu, menu_command, device_id, icon_type, spaces)
                        .await?;
                }
                if !self.running {
                    return Ok(false);
                }
                if !self.interactive {
                    self.running = false;
                }
                Ok(false)
            }
            Some(DeviceMenuOptions::AdjustVolume) => {
                self.handle_volume_menu(menu, menu_command, node, icon_type, spaces, is_output)
                    .await?;
                if !self.running {
                    return Ok(false);
                }
                if !self.interactive {
                    self.running = false;
                }
                Ok(false)
            }
            Some(DeviceMenuOptions::Back) => Ok(false),
            None => {
                if !self.interactive {
                    self.running = false;
                }
                debug!(
                    "Exited device menu for {}",
                    node.description.as_ref().unwrap_or(&node.name)
                );
                Ok(false)
            }
        }
    }

    async fn handle_profile_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        device_id: u32,
        icon_type: &str,
        spaces: usize,
    ) -> Result<()> {
        let mut stay_in_profile_menu = true;

        while stay_in_profile_menu {
            let should_stay = self
                .handle_profile_options(menu, menu_command, device_id, icon_type, spaces)
                .await?;

            if !should_stay {
                stay_in_profile_menu = false;
            }
        }

        Ok(())
    }

    async fn handle_profile_options(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        device_id: u32,
        icon_type: &str,
        spaces: usize,
    ) -> Result<bool> {
        let profiles = self.controller.get_device_profiles(device_id);
        let current_profile = self.controller.get_device_current_profile(device_id);

        let device_name = self.controller.get_device_name(device_id);

        let option = menu
            .show_profile_menu(
                menu_command,
                icon_type,
                spaces,
                &device_name,
                &profiles,
                current_profile.as_ref().map(|p| p.index),
                self.interactive,
            )
            .await?;

        match option {
            Some(ProfileMenuOptions::SelectProfile(profile_index)) => {
                let target_profile = profile_index;
                self.perform_profile_switch(device_id, profile_index, &device_name, &profiles)
                    .await?;
                self.wait_for_profile_change(device_id, target_profile)
                    .await?;
                if !self.interactive {
                    self.running = false;
                }
                Ok(false)
            }
            Some(ProfileMenuOptions::Back) => Ok(false),
            None => {
                if !self.interactive {
                    self.running = false;
                }
                debug!("Exited profile menu for {device_name}");
                Ok(false)
            }
        }
    }

    async fn wait_for_profile_change(
        &self,
        device_id: u32,
        target_profile_index: u32,
    ) -> Result<()> {
        for _ in 0..20 {
            if let Some(current_profile) = self.controller.get_device_current_profile(device_id) {
                if current_profile.index == target_profile_index {
                    return Ok(());
                }
            }
            sleep(Duration::from_millis(50)).await;
        }

        Ok(())
    }

    async fn handle_volume_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        node: &Node,
        icon_type: &str,
        spaces: usize,
        is_output: bool,
    ) -> Result<()> {
        let mut stay_in_volume_menu = true;
        let mut current_node = node.clone();
        let mut last_action: Option<VolumeMenuOptions> = None;

        while stay_in_volume_menu {
            if let Some(updated_node) = self.controller.get_node(current_node.id) {
                current_node = updated_node;
            }

            let (should_stay, selected_action) = self
                .handle_volume_options(
                    menu,
                    menu_command,
                    &current_node,
                    icon_type,
                    spaces,
                    is_output,
                    last_action,
                )
                .await?;

            last_action = selected_action;

            if !should_stay {
                stay_in_volume_menu = false;
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_volume_options(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        node: &Node,
        icon_type: &str,
        spaces: usize,
        is_output: bool,
        last_action: Option<VolumeMenuOptions>,
    ) -> Result<(bool, Option<VolumeMenuOptions>)> {
        let device_name = if node.device_id.is_some() {
            self.controller.get_device_name(node.device_id.unwrap_or(0))
        } else {
            menu.format_stream_display_name(node, &self.controller)
        };

        let volume_display = if node.volume.muted {
            t!("menus.volume.muted").to_string()
        } else {
            format!("{}%", node.volume.percent())
        };

        let step_percent = (self.volume_step * 100.0).round() as u8;
        let option = menu
            .show_volume_menu(
                menu_command,
                icon_type,
                spaces,
                node,
                is_output,
                last_action,
                &device_name,
                &volume_display,
                step_percent,
                self.interactive,
            )
            .await?;

        match option {
            Some(VolumeMenuOptions::Increase) => {
                self.perform_volume_change(node, self.volume_step).await?;
                Ok((true, Some(VolumeMenuOptions::Increase)))
            }
            Some(VolumeMenuOptions::Decrease) => {
                self.perform_volume_change(node, -self.volume_step).await?;
                Ok((true, Some(VolumeMenuOptions::Decrease)))
            }
            Some(VolumeMenuOptions::Mute) => {
                self.perform_mute_toggle(node, true).await?;
                Ok((true, Some(VolumeMenuOptions::Mute)))
            }
            Some(VolumeMenuOptions::Unmute) => {
                self.perform_mute_toggle(node, false).await?;
                Ok((true, Some(VolumeMenuOptions::Unmute)))
            }
            Some(VolumeMenuOptions::Back) => Ok((false, None)),
            None => {
                if !self.interactive {
                    self.running = false;
                }
                debug!(
                    "Exited volume menu for {}",
                    node.description.as_ref().unwrap_or(&node.name)
                );
                Ok((false, None))
            }
        }
    }

    async fn perform_set_default(&self, node: &Node, is_output: bool) -> Result<()> {
        let device_type = if is_output { "output" } else { "input" };

        let result = if is_output {
            self.controller.set_default_sink(node.id).await
        } else {
            self.controller.set_default_source(node.id).await
        };

        let display_name = self.controller.get_node_base_name(node);

        match result {
            Ok(()) => {
                let msg = t!(
                    "notifications.pw.default_set",
                    device_type = device_type,
                    device_name = display_name
                );
                info!("{msg}");
                self.notification_manager
                    .send_default_changed_notification(device_type, &display_name)?;
            }
            Err(e) => {
                let msg = e.to_string();
                info!("{msg}");
                try_send_notification!(
                    self.notification_manager,
                    None,
                    Some(msg),
                    Some("output"),
                    None
                );
            }
        }

        Ok(())
    }

    async fn perform_profile_switch(
        &self,
        device_id: u32,
        profile_index: u32,
        device_name: &str,
        profiles: &[Profile],
    ) -> Result<()> {
        match self
            .controller
            .switch_device_profile(device_id, profile_index)
            .await
        {
            Ok(()) => {
                if let Some(profile) = profiles.iter().find(|p| p.index == profile_index) {
                    let msg = t!(
                        "notifications.pw.profile_switched",
                        device_name = device_name,
                        profile_name = &profile.description
                    );
                    info!("{msg}");
                    try_send_notification!(
                        self.notification_manager,
                        None,
                        Some(msg.to_string()),
                        Some("switch_profile"),
                        None
                    );
                }
            }
            Err(e) => {
                let msg = e.to_string();
                info!("{msg}");
                try_send_notification!(
                    self.notification_manager,
                    None,
                    Some(msg),
                    Some("switch_profile"),
                    None
                );
            }
        }

        Ok(())
    }

    async fn perform_volume_change(&self, node: &Node, delta: f32) -> Result<()> {
        let new_volume = (node.volume.linear + delta).clamp(0.0, 2.0);

        if node.volume.muted {
            self.controller.set_mute(node.id, false).await?;
        }

        self.controller.set_volume(node.id, new_volume).await?;

        let volume_percent = (new_volume * 100.0).round() as u8;
        let display_name = self.controller.get_node_base_name(node);

        let msg = t!(
            "notifications.pw.volume_changed",
            device_name = display_name,
            volume = volume_percent
        );

        info!("{msg}");
        self.notification_manager.send_volume_notification(
            &display_name,
            volume_percent,
            false,
            &node.node_type,
        )?;

        Ok(())
    }

    async fn perform_mute_toggle(&self, node: &Node, mute: bool) -> Result<()> {
        self.controller.set_mute(node.id, mute).await?;

        let display_name = if node.device_id.is_some() {
            self.controller.get_device_name(node.device_id.unwrap_or(0))
        } else if let Some(media_name) = self.controller.get_media_name(node) {
            format!(
                "{} - {}",
                self.controller.get_application_name(node),
                media_name
            )
        } else {
            self.controller.get_application_name(node)
        };

        let summary = if mute {
            t!("notifications.pw.device_muted")
        } else {
            t!("notifications.pw.device_unmuted")
        };

        info!("{} {}", &summary, &display_name);
        self.notification_manager.send_volume_notification(
            &display_name,
            node.volume.percent(),
            mute,
            &node.node_type,
        )?;

        Ok(())
    }

    async fn perform_sample_rate_change(&self, sample_rate: u32) -> Result<()> {
        self.controller.set_sample_rate(sample_rate).await?;

        let rate_khz = sample_rate as f32 / 1000.0;
        let msg = t!(
            "notifications.pw.sample_rate_changed",
            sample_rate = format!("{:.1} kHz", rate_khz)
        );

        info!("{msg}");
        try_send_notification!(
            self.notification_manager,
            Some("Sample Rate Changed".to_string()),
            Some(msg.to_string()),
            Some("profile"),
            None
        );

        Ok(())
    }
}
