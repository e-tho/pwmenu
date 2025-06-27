use crate::{
    icons::Icons,
    launcher::{Launcher, LauncherType},
    pw::{controller::Controller, nodes::Node, Profile},
};
use anyhow::Result;
use rust_i18n::t;
use std::borrow::Cow;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum MainMenuOptions {
    ShowOutputMenu,
    ShowInputMenu,
}

impl MainMenuOptions {
    pub fn from_string(option: &str) -> Option<Self> {
        match option {
            s if s == t!("menus.main.options.outputs.name") => {
                Some(MainMenuOptions::ShowOutputMenu)
            }
            s if s == t!("menus.main.options.inputs.name") => Some(MainMenuOptions::ShowInputMenu),
            _ => None,
        }
    }

    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            MainMenuOptions::ShowOutputMenu => t!("menus.main.options.outputs.name"),
            MainMenuOptions::ShowInputMenu => t!("menus.main.options.inputs.name"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum OutputMenuOptions {
    RefreshList,
    Device(String),
}

impl OutputMenuOptions {
    pub fn from_string(option: &str) -> Option<Self> {
        match option {
            s if s == t!("menus.output.options.refresh.name") => {
                Some(OutputMenuOptions::RefreshList)
            }
            other => Some(OutputMenuOptions::Device(other.to_string())),
        }
    }

    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            OutputMenuOptions::RefreshList => t!("menus.output.options.refresh.name"),
            OutputMenuOptions::Device(_) => t!("menus.output.options.device.name"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum InputMenuOptions {
    RefreshList,
    Device(String),
}

impl InputMenuOptions {
    pub fn from_string(option: &str) -> Option<Self> {
        match option {
            s if s == t!("menus.input.options.refresh.name") => Some(InputMenuOptions::RefreshList),
            other => Some(InputMenuOptions::Device(other.to_string())),
        }
    }

    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            InputMenuOptions::RefreshList => t!("menus.input.options.refresh.name"),
            InputMenuOptions::Device(_) => t!("menus.input.options.device.name"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfileMenuOptions {
    SelectProfile(u32),
}

impl ProfileMenuOptions {
    pub fn from_string_with_profiles(option: &str, profiles: &[Profile]) -> Option<Self> {
        profiles
            .iter()
            .find(|profile| profile.description == option)
            .map(|profile| ProfileMenuOptions::SelectProfile(profile.index))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceMenuOptions {
    SetDefault,
    SwitchProfile,
    AdjustVolume,
}

impl DeviceMenuOptions {
    pub fn from_string(option: &str) -> Option<Self> {
        match option {
            s if s == t!("menus.device.options.set_default.name") => {
                Some(DeviceMenuOptions::SetDefault)
            }
            s if s == t!("menus.device.options.switch_profile.name") => {
                Some(DeviceMenuOptions::SwitchProfile)
            }
            s if s == t!("menus.device.options.adjust_volume.name") => {
                Some(DeviceMenuOptions::AdjustVolume)
            }
            _ => None,
        }
    }

    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            DeviceMenuOptions::SetDefault => t!("menus.device.options.set_default.name"),
            DeviceMenuOptions::SwitchProfile => t!("menus.device.options.switch_profile.name"),
            DeviceMenuOptions::AdjustVolume => t!("menus.device.options.adjust_volume.name"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VolumeMenuOptions {
    Increase,
    Decrease,
    Mute,
    Unmute,
}

impl VolumeMenuOptions {
    pub fn from_string(option: &str) -> Option<Self> {
        match option {
            s if s == t!("menus.volume.options.increase.name") => Some(VolumeMenuOptions::Increase),
            s if s == t!("menus.volume.options.decrease.name") => Some(VolumeMenuOptions::Decrease),
            s if s == t!("menus.volume.options.mute.name") => Some(VolumeMenuOptions::Mute),
            s if s == t!("menus.volume.options.unmute.name") => Some(VolumeMenuOptions::Unmute),
            _ => None,
        }
    }

    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            VolumeMenuOptions::Increase => t!("menus.volume.options.increase.name"),
            VolumeMenuOptions::Decrease => t!("menus.volume.options.decrease.name"),
            VolumeMenuOptions::Mute => t!("menus.volume.options.mute.name"),
            VolumeMenuOptions::Unmute => t!("menus.volume.options.unmute.name"),
        }
    }
}

#[derive(Clone)]
pub struct Menu {
    pub launcher_type: LauncherType,
    pub icons: Arc<Icons>,
}

impl Menu {
    pub fn new(launcher_type: LauncherType, icons: Arc<Icons>) -> Self {
        Self {
            launcher_type,
            icons,
        }
    }

    pub fn run_launcher(
        &self,
        launcher_command: &Option<String>,
        input: Option<&str>,
        icon_type: &str,
        prompt: Option<&str>,
    ) -> Result<Option<String>> {
        let cmd = Launcher::create_command(
            &self.launcher_type,
            launcher_command,
            icon_type,
            prompt,
            prompt,
        )?;

        Launcher::run(cmd, input)
    }

    pub fn clean_menu_output(&self, output: &str, icon_type: &str) -> String {
        let output_trimmed = output.trim();

        if icon_type == "font" {
            output_trimmed
                .chars()
                .skip_while(|c| !c.is_ascii_alphanumeric())
                .collect::<String>()
                .trim()
                .to_string()
        } else if icon_type == "xdg" {
            output_trimmed
                .split('\0')
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        } else {
            output_trimmed.to_string()
        }
    }

    pub fn get_icon_text<T>(&self, items: Vec<(&str, T)>, icon_type: &str, spaces: usize) -> String
    where
        T: AsRef<str>,
    {
        items
            .into_iter()
            .map(|(icon_key, text)| {
                let icon = self.icons.get_icon(icon_key, icon_type);
                let text = text.as_ref();
                match icon_type {
                    "font" => format!("{}{}{}", icon, " ".repeat(spaces), text),
                    "xdg" => format!("{text}\0icon\x1f{icon}"),
                    _ => text.to_string(),
                }
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn format_node_display(
        &self,
        node: &Node,
        controller: &Controller,
        icon_type: &str,
        spaces: usize,
    ) -> String {
        let mut display_name = node.description.as_ref().unwrap_or(&node.name).clone();

        if let Some(app_name) = &node.application_name {
            display_name = format!("{display_name} ({app_name})");
        }

        let volume_str = format!(" [{}%]", node.volume.percent());
        display_name.push_str(&volume_str);

        if node.is_default {
            display_name.push_str(&format!(" {}", self.icons.get_icon("default", "generic")));
        }

        let device_info = controller.get_device_info(node);
        let icon = self.icons.get_device_icon(&device_info, icon_type);

        self.format_display_with_icon(&display_name, &icon, icon_type, spaces)
    }

    pub fn format_display_with_icon(
        &self,
        text: &str,
        icon: &str,
        icon_type: &str,
        spaces: usize,
    ) -> String {
        match icon_type {
            "xdg" => format!("{text}\0icon\x1f{icon}"),
            "font" | "generic" => format!("{}{}{}", icon, " ".repeat(spaces), text),
            _ => text.to_string(),
        }
    }

    pub async fn show_main_menu(
        &self,
        launcher_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<MainMenuOptions>> {
        let options = vec![
            ("output", MainMenuOptions::ShowOutputMenu.to_str()),
            ("input", MainMenuOptions::ShowInputMenu.to_str()),
        ];

        let input = self.get_icon_text(options, icon_type, spaces);

        let menu_output = self.run_launcher(launcher_command, Some(&input), icon_type, None)?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(MainMenuOptions::from_string(&cleaned_output));
        }

        Ok(None)
    }

    pub async fn show_output_menu(
        &self,
        launcher_command: &Option<String>,
        controller: &Controller,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<OutputMenuOptions>> {
        let refresh_text = OutputMenuOptions::RefreshList.to_str();
        let options_start = vec![("refresh", refresh_text.as_ref())];

        let mut input = self.get_icon_text(options_start, icon_type, spaces);

        let output_nodes = controller.get_output_nodes();

        for node in output_nodes {
            let node_display = self.format_node_display(&node, controller, icon_type, spaces);
            input.push_str(&format!("\n{node_display}"));
        }

        let prompt = t!("menus.output.prompt");
        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&prompt))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);

            if cleaned_output == refresh_text.as_ref() {
                return Ok(Some(OutputMenuOptions::RefreshList));
            } else {
                return Ok(Some(OutputMenuOptions::Device(cleaned_output)));
            }
        }

        Ok(None)
    }

    pub async fn show_input_menu(
        &self,
        launcher_command: &Option<String>,
        controller: &Controller,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<InputMenuOptions>> {
        let refresh_text = InputMenuOptions::RefreshList.to_str();
        let options_start = vec![("refresh", refresh_text.as_ref())];

        let mut input = self.get_icon_text(options_start, icon_type, spaces);

        let input_nodes = controller.get_input_nodes();

        for node in input_nodes {
            let node_display = self.format_node_display(&node, controller, icon_type, spaces);
            input.push_str(&format!("\n{node_display}"));
        }

        let prompt = t!("menus.input.prompt");
        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&prompt))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);

            if cleaned_output == refresh_text.as_ref() {
                return Ok(Some(InputMenuOptions::RefreshList));
            } else {
                return Ok(Some(InputMenuOptions::Device(cleaned_output)));
            }
        }

        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn show_device_options(
        &self,
        launcher_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
        device_name: &str,
        is_default: bool,
        is_output_menu: bool,
        has_profiles: bool,
    ) -> Result<Option<DeviceMenuOptions>> {
        let mut options = Vec::new();

        if !is_default {
            options.push(("set_default", DeviceMenuOptions::SetDefault.to_str()));
        }

        if has_profiles {
            options.push(("switch_profile", DeviceMenuOptions::SwitchProfile.to_str()));
        }

        let volume_icon_key = if is_output_menu {
            "output_volume"
        } else {
            "input_volume"
        };

        options.push((volume_icon_key, DeviceMenuOptions::AdjustVolume.to_str()));

        let input = self.get_icon_text(options, icon_type, spaces);
        let prompt = t!("menus.device.prompt", device_name = device_name);

        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&prompt))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(DeviceMenuOptions::from_string(&cleaned_output));
        }

        Ok(None)
    }

    pub async fn show_profile_menu(
        &self,
        launcher_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
        device_name: &str,
        profiles: &[Profile],
        current_profile_index: Option<u32>,
    ) -> Result<Option<ProfileMenuOptions>> {
        if profiles.is_empty() {
            return Ok(None);
        }

        let mut options = Vec::new();

        for profile in profiles {
            let mut display_name = profile.description.clone();

            if Some(profile.index) == current_profile_index {
                display_name.push_str(&format!(" {}", self.icons.get_icon("default", "generic")));
            }

            options.push(("profile", display_name));
        }

        let input = self.get_icon_text(options, icon_type, spaces);
        let prompt = t!("menus.profile.prompt", device_name = device_name);

        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&prompt))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(ProfileMenuOptions::from_string_with_profiles(
                &cleaned_output,
                profiles,
            ));
        }

        Ok(None)
    }

    pub async fn show_volume_menu(
        &self,
        launcher_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
        node: &Node,
        is_output_menu: bool,
        last_action: Option<VolumeMenuOptions>,
    ) -> Result<Option<VolumeMenuOptions>> {
        let mut options = Vec::new();

        let increase_key = if is_output_menu {
            "output_volume_up"
        } else {
            "input_volume_up"
        };
        let decrease_key = if is_output_menu {
            "output_volume_down"
        } else {
            "input_volume_down"
        };

        match last_action {
            Some(VolumeMenuOptions::Decrease) => {
                options.push((decrease_key, VolumeMenuOptions::Decrease.to_str()));
                options.push((increase_key, VolumeMenuOptions::Increase.to_str()));
            }
            Some(VolumeMenuOptions::Increase) => {
                options.push((increase_key, VolumeMenuOptions::Increase.to_str()));
                options.push((decrease_key, VolumeMenuOptions::Decrease.to_str()));
            }
            _ => {
                options.push((increase_key, VolumeMenuOptions::Increase.to_str()));
                options.push((decrease_key, VolumeMenuOptions::Decrease.to_str()));
            }
        }

        if node.volume.muted {
            let unmute_key = if is_output_menu {
                "output_unmute"
            } else {
                "input_unmute"
            };
            options.push((unmute_key, VolumeMenuOptions::Unmute.to_str()));
        } else {
            let mute_key = if is_output_menu {
                "output_mute"
            } else {
                "input_mute"
            };
            options.push((mute_key, VolumeMenuOptions::Mute.to_str()));
        }

        let input = self.get_icon_text(options, icon_type, spaces);
        let volume_percent = node.volume.percent();
        let prompt = t!(
            "menus.volume.prompt",
            device_name = node.description.as_ref().unwrap_or(&node.name),
            volume = volume_percent
        );

        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&prompt))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(VolumeMenuOptions::from_string(&cleaned_output));
        }

        Ok(None)
    }
}
