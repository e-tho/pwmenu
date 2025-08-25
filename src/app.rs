use crate::{
    icons::Icons,
    menu::{
        DeviceMenuOptions, InputMenuOptions, MainMenuOptions, Menu, OutputMenuOptions,
        ProfileMenuOptions, VolumeMenuOptions,
    },
    notification::NotificationManager,
    pw::{controller::Controller, nodes::Node, Profile},
};
use anyhow::anyhow;
use anyhow::Result;
use rust_i18n::t;
use std::sync::Arc;
use tokio::{
    sync::mpsc::UnboundedSender,
    time::{sleep, Duration},
};

const VOLUME_STEP: f32 = 0.05; // 5% volume change per step

pub struct App {
    pub running: bool,
    controller: Controller,
    log_sender: UnboundedSender<String>,
    notification_manager: Arc<NotificationManager>,
}

impl App {
    pub async fn new(
        _menu: Menu,
        log_sender: UnboundedSender<String>,
        icons: Arc<Icons>,
    ) -> Result<Self> {
        let controller = Controller::new(log_sender.clone()).await?;
        let notification_manager = Arc::new(NotificationManager::new(icons.clone()));

        try_send_log!(log_sender, t!("notifications.pw.initialized").to_string());

        Ok(Self {
            running: true,
            controller,
            log_sender,
            notification_manager,
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
                    try_send_log!(
                        self.log_sender,
                        t!("notifications.pw.main_menu_exited").to_string()
                    );
                    self.running = false;
                }
            }
        }

        Ok(None)
    }

    pub async fn run_output_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<String>> {
        self.handle_output_menu(menu, menu_command, icon_type, spaces)
            .await?;
        Ok(None)
    }

    pub async fn run_input_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<String>> {
        self.handle_input_menu(menu, menu_command, icon_type, spaces)
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
            MainMenuOptions::ShowOutputMenu => {
                self.handle_output_menu(menu, menu_command, icon_type, spaces)
                    .await?;
            }
            MainMenuOptions::ShowInputMenu => {
                self.handle_input_menu(menu, menu_command, icon_type, spaces)
                    .await?;
            }
        }
        Ok(None)
    }

    async fn handle_output_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<()> {
        let mut stay_in_output_menu = true;

        while stay_in_output_menu {
            let should_stay = self
                .handle_output_options(menu, menu_command, icon_type, spaces)
                .await?;

            if !should_stay {
                stay_in_output_menu = false;
            }
        }

        Ok(())
    }

    async fn handle_output_options(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<bool> {
        let nodes = self.controller.get_output_nodes();
        let menu_result = menu
            .show_output_menu(menu_command, &nodes, &self.controller, icon_type, spaces)
            .await?;

        match menu_result {
            Some(selection) => {
                let refresh_text = OutputMenuOptions::RefreshList.to_str();
                if selection == refresh_text.as_ref() {
                    try_send_log!(
                        self.log_sender,
                        t!("notifications.pw.outputs_refreshed").to_string()
                    );
                    try_send_notification!(
                        self.notification_manager,
                        Some(t!("notifications.pw.outputs_refreshed").to_string()),
                        None,
                        Some("refresh"),
                        None
                    );
                    Ok(true)
                } else {
                    let selected_node =
                        self.handle_device_selection(&nodes, &selection, menu, icon_type, spaces)?;
                    if let Some(node) = selected_node {
                        self.handle_device_menu(menu, menu_command, &node, icon_type, spaces, true)
                            .await?;
                    }
                    Ok(true)
                }
            }
            None => {
                try_send_log!(
                    self.log_sender,
                    t!("notifications.pw.output_menu_exited").to_string()
                );
                Ok(false)
            }
        }
    }

    async fn handle_input_menu(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<()> {
        let mut stay_in_input_menu = true;

        while stay_in_input_menu {
            let should_stay = self
                .handle_input_options(menu, menu_command, icon_type, spaces)
                .await?;

            if !should_stay {
                stay_in_input_menu = false;
            }
        }

        Ok(())
    }

    async fn handle_input_options(
        &mut self,
        menu: &Menu,
        menu_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<bool> {
        let nodes = self.controller.get_input_nodes();
        let menu_result = menu
            .show_input_menu(menu_command, &nodes, &self.controller, icon_type, spaces)
            .await?;

        match menu_result {
            Some(selection) => {
                let refresh_text = InputMenuOptions::RefreshList.to_str();
                if selection == refresh_text.as_ref() {
                    try_send_log!(
                        self.log_sender,
                        t!("notifications.pw.inputs_refreshed").to_string()
                    );
                    try_send_notification!(
                        self.notification_manager,
                        Some(t!("notifications.pw.inputs_refreshed").to_string()),
                        None,
                        Some("refresh"),
                        None
                    );
                    Ok(true)
                } else {
                    let selected_node =
                        self.handle_device_selection(&nodes, &selection, menu, icon_type, spaces)?;
                    if let Some(node) = selected_node {
                        self.handle_device_menu(menu, menu_command, &node, icon_type, spaces, true)
                            .await?;
                    }
                    Ok(true)
                }
            }
            None => {
                try_send_log!(
                    self.log_sender,
                    t!("notifications.pw.input_menu_exited").to_string()
                );
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
                current_node = updated_node;
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
            )
            .await?;

        if let Some(option) = option {
            match option {
                DeviceMenuOptions::SetDefault => {
                    self.perform_set_default(node, is_output).await?;
                    Ok(true)
                }
                DeviceMenuOptions::SwitchProfile => {
                    if let Some(device_id) = node.device_id {
                        self.handle_profile_menu(menu, menu_command, device_id, icon_type, spaces)
                            .await?;
                    }
                    Ok(true)
                }
                DeviceMenuOptions::AdjustVolume => {
                    self.handle_volume_menu(menu, menu_command, node, icon_type, spaces, is_output)
                        .await?;
                    Ok(true)
                }
            }
        } else {
            try_send_log!(
                self.log_sender,
                format!(
                    "Exited device menu for {}",
                    node.description.as_ref().unwrap_or(&node.name)
                )
            );
            Ok(false)
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
            )
            .await?;

        if let Some(ProfileMenuOptions::SelectProfile(profile_index)) = option {
            let target_profile = profile_index;

            self.perform_profile_switch(device_id, profile_index, &device_name, &profiles)
                .await?;

            self.wait_for_profile_change(device_id, target_profile)
                .await?;

            Ok(true)
        } else {
            try_send_log!(
                self.log_sender,
                format!("Exited profile menu for {device_name}")
            );
            Ok(false)
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
        let device_name = self.controller.get_device_name(node.device_id.unwrap_or(0));

        let volume_display = if node.volume.muted {
            t!("menus.volume.muted").to_string()
        } else {
            format!("{}%", node.volume.percent())
        };

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
            )
            .await?;

        if let Some(selected_option) = option {
            match selected_option {
                VolumeMenuOptions::Increase => {
                    self.perform_volume_change(node, VOLUME_STEP).await?;
                }
                VolumeMenuOptions::Decrease => {
                    self.perform_volume_change(node, -VOLUME_STEP).await?;
                }
                VolumeMenuOptions::Mute => {
                    self.perform_mute_toggle(node, true).await?;
                }
                VolumeMenuOptions::Unmute => {
                    self.perform_mute_toggle(node, false).await?;
                }
            }
            Ok((true, Some(selected_option)))
        } else {
            try_send_log!(
                self.log_sender,
                format!(
                    "Exited volume menu for {}",
                    node.description.as_ref().unwrap_or(&node.name)
                )
            );
            Ok((false, None))
        }
    }

    fn get_display_name(node: &Node) -> &str {
        node.description.as_ref().unwrap_or(&node.name)
    }

    async fn perform_set_default(&self, node: &Node, is_output: bool) -> Result<()> {
        let device_type = if is_output { "output" } else { "input" };

        if is_output {
            self.controller.set_default_sink(node.id).await?;
        } else {
            self.controller.set_default_source(node.id).await?;
        }

        let display_name = Self::get_display_name(node);
        let msg = t!(
            "notifications.pw.default_set",
            device_type = device_type,
            device_name = display_name
        );

        try_send_log!(self.log_sender, msg.to_string());
        self.notification_manager
            .send_default_changed_notification(device_type, display_name)?;

        Ok(())
    }

    async fn perform_profile_switch(
        &self,
        device_id: u32,
        profile_index: u32,
        device_name: &str,
        profiles: &[Profile],
    ) -> Result<()> {
        self.controller
            .switch_device_profile(device_id, profile_index)
            .await?;

        if let Some(profile) = profiles.iter().find(|p| p.index == profile_index) {
            let msg = t!(
                "notifications.pw.profile_switched",
                device_name = device_name,
                profile_name = &profile.description
            );

            try_send_log!(self.log_sender, msg.to_string());
            try_send_notification!(
                self.notification_manager,
                None,
                Some(msg.to_string()),
                Some("switch_profile"),
                None
            );
        }

        Ok(())
    }

    async fn perform_volume_change(&self, node: &Node, delta: f32) -> Result<()> {
        let node_id = node.id;
        let current_node = self
            .controller
            .get_node(node.id)
            .ok_or_else(|| anyhow!("Node {node_id} not found"))?;

        let current = current_node.volume.linear;
        let new_volume = (current + delta).clamp(0.0, 1.0);

        self.controller.set_volume(node.id, new_volume).await?;

        let volume_percent = (new_volume * 100.0).round() as u8;
        let display_name = current_node
            .description
            .as_ref()
            .unwrap_or(&current_node.name);

        let msg = t!(
            "notifications.pw.volume_changed",
            device_name = display_name,
            volume = volume_percent
        );

        try_send_log!(self.log_sender, msg.to_string());
        self.notification_manager.send_volume_notification(
            display_name,
            volume_percent,
            current_node.volume.muted,
            &current_node.node_type,
        )?;

        Ok(())
    }

    async fn perform_mute_toggle(&self, node: &Node, mute: bool) -> Result<()> {
        self.controller.set_mute(node.id, mute).await?;

        let display_name = node.description.as_ref().unwrap_or(&node.name);
        let msg = if mute {
            t!("notifications.pw.device_muted", device_name = display_name)
        } else {
            t!(
                "notifications.pw.device_unmuted",
                device_name = display_name
            )
        };

        try_send_log!(self.log_sender, msg.to_string());
        self.notification_manager.send_volume_notification(
            display_name,
            node.volume.percent(),
            mute,
            &node.node_type,
        )?;

        Ok(())
    }
}
