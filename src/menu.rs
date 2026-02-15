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
    ShowOutputDeviceMenu,
    ShowInputDeviceMenu,
    ShowOutputStreamsMenu,
    ShowInputStreamsMenu,
    ShowSettingsMenu,
}

impl MainMenuOptions {
    pub fn from_string(option: &str) -> Option<Self> {
        match option {
            s if s == t!("menus.main.options.output_devices.name") => {
                Some(MainMenuOptions::ShowOutputDeviceMenu)
            }
            s if s == t!("menus.main.options.input_devices.name") => {
                Some(MainMenuOptions::ShowInputDeviceMenu)
            }
            s if s == t!("menus.main.options.output_streams.name") => {
                Some(MainMenuOptions::ShowOutputStreamsMenu)
            }
            s if s == t!("menus.main.options.input_streams.name") => {
                Some(MainMenuOptions::ShowInputStreamsMenu)
            }
            s if s == t!("menus.main.options.settings.name") => {
                Some(MainMenuOptions::ShowSettingsMenu)
            }
            _ => None,
        }
    }

    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            MainMenuOptions::ShowOutputDeviceMenu => t!("menus.main.options.output_devices.name"),
            MainMenuOptions::ShowInputDeviceMenu => t!("menus.main.options.input_devices.name"),
            MainMenuOptions::ShowOutputStreamsMenu => t!("menus.main.options.output_streams.name"),
            MainMenuOptions::ShowInputStreamsMenu => t!("menus.main.options.input_streams.name"),
            MainMenuOptions::ShowSettingsMenu => t!("menus.main.options.settings.name"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsMenuOptions {
    SetSampleRate,
    Back,
}

impl SettingsMenuOptions {
    pub fn from_string(option: &str) -> Option<Self> {
        match option {
            s if s == t!("menus.settings.options.set_sample_rate.name") => {
                Some(SettingsMenuOptions::SetSampleRate)
            }
            s if s == t!("menus.common.back") => Some(SettingsMenuOptions::Back),
            _ => None,
        }
    }

    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            SettingsMenuOptions::SetSampleRate => {
                t!("menus.settings.options.set_sample_rate.name")
            }
            SettingsMenuOptions::Back => t!("menus.common.back"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StreamMenuOptions {
    RefreshList,
    Stream(String),
}

impl StreamMenuOptions {
    pub fn from_string(option: &str) -> Option<Self> {
        match option {
            s if s == t!("menus.streams.options.refresh.name") => {
                Some(StreamMenuOptions::RefreshList)
            }
            other => Some(StreamMenuOptions::Stream(other.to_string())),
        }
    }

    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            StreamMenuOptions::RefreshList => t!("menus.streams.options.refresh.name"),
            StreamMenuOptions::Stream(_) => t!("menus.streams.options.stream.name"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum OutputDeviceMenuOptions {
    RefreshList,
    Device(String),
}

impl OutputDeviceMenuOptions {
    pub fn from_string(option: &str) -> Option<Self> {
        match option {
            s if s == t!("menus.output_devices.options.refresh.name") => {
                Some(OutputDeviceMenuOptions::RefreshList)
            }
            other => Some(OutputDeviceMenuOptions::Device(other.to_string())),
        }
    }

    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            OutputDeviceMenuOptions::RefreshList => t!("menus.output_devices.options.refresh.name"),
            OutputDeviceMenuOptions::Device(_) => t!("menus.output_devices.options.device.name"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum InputDeviceMenuOptions {
    RefreshList,
    Device(String),
}

impl InputDeviceMenuOptions {
    pub fn from_string(option: &str) -> Option<Self> {
        match option {
            s if s == t!("menus.input_devices.options.refresh.name") => {
                Some(InputDeviceMenuOptions::RefreshList)
            }
            other => Some(InputDeviceMenuOptions::Device(other.to_string())),
        }
    }

    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            InputDeviceMenuOptions::RefreshList => t!("menus.input_devices.options.refresh.name"),
            InputDeviceMenuOptions::Device(_) => t!("menus.input_devices.options.device.name"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfileMenuOptions {
    SelectProfile(u32),
    Back,
}

impl ProfileMenuOptions {
    pub fn from_string_with_profiles(option: &str, profiles: &[Profile]) -> Option<Self> {
        if option == t!("menus.common.back") {
            return Some(ProfileMenuOptions::Back);
        }

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
    Back,
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
            s if s == t!("menus.common.back") => Some(DeviceMenuOptions::Back),
            _ => None,
        }
    }

    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            DeviceMenuOptions::SetDefault => t!("menus.device.options.set_default.name"),
            DeviceMenuOptions::SwitchProfile => t!("menus.device.options.switch_profile.name"),
            DeviceMenuOptions::AdjustVolume => t!("menus.device.options.adjust_volume.name"),
            DeviceMenuOptions::Back => t!("menus.common.back"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VolumeMenuOptions {
    Increase,
    Decrease,
    Mute,
    Unmute,
    Back,
}

impl VolumeMenuOptions {
    pub fn from_string(option: &str, step_percent: u8) -> Option<Self> {
        let increase_text = t!("menus.volume.options.increase.name", step = step_percent);
        let decrease_text = t!("menus.volume.options.decrease.name", step = step_percent);

        match option {
            s if s == increase_text => Some(VolumeMenuOptions::Increase),
            s if s == decrease_text => Some(VolumeMenuOptions::Decrease),
            s if s == t!("menus.volume.options.mute.name") => Some(VolumeMenuOptions::Mute),
            s if s == t!("menus.volume.options.unmute.name") => Some(VolumeMenuOptions::Unmute),
            s if s == t!("menus.common.back") => Some(VolumeMenuOptions::Back),
            _ => None,
        }
    }

    pub fn to_str(&self, step_percent: Option<u8>) -> Cow<'static, str> {
        match self {
            VolumeMenuOptions::Increase => {
                let step = step_percent.unwrap_or(5);
                t!("menus.volume.options.increase.name", step = step)
            }
            VolumeMenuOptions::Decrease => {
                let step = step_percent.unwrap_or(5);
                t!("menus.volume.options.decrease.name", step = step)
            }
            VolumeMenuOptions::Mute => t!("menus.volume.options.mute.name"),
            VolumeMenuOptions::Unmute => t!("menus.volume.options.unmute.name"),
            VolumeMenuOptions::Back => t!("menus.common.back"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampleRateMenuOptions {
    SelectRate(u32),
    Back,
}

impl SampleRateMenuOptions {
    pub fn from_string_with_rates(option: &str, rates: &[u32]) -> Option<Self> {
        if option == t!("menus.common.back") {
            return Some(SampleRateMenuOptions::Back);
        }

        for &rate in rates {
            let display_text = format!("{:.1} kHz", rate as f32 / 1000.0);
            if option == display_text {
                return Some(SampleRateMenuOptions::SelectRate(rate));
            }
        }
        None
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
        hint: Option<&str>,
    ) -> Result<Option<String>> {
        let cmd = Launcher::create_command(&self.launcher_type, launcher_command, icon_type, hint)?;

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
        let mut display_name = controller.get_node_base_name(node);

        if let Some(app_name) = &node.application_name {
            display_name = format!("{display_name} ({app_name})");
        }

        if let Some(port_number) = controller.get_node_port_number(node) {
            display_name.push_str(&format!(" - {port_number}"));
        }

        let volume_str = if node.volume.muted {
            format!(" [{}]", t!("menus.volume.muted"))
        } else {
            format!(" [{}%]", node.volume.percent())
        };
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

    pub fn format_stream_display_name(&self, node: &Node, controller: &Controller) -> String {
        let app_name = controller.get_application_name(node);

        if let Some(media_name) = controller.get_media_name(node) {
            format!("{app_name} - {media_name}")
        } else {
            app_name
        }
    }

    pub async fn show_main_menu(
        &self,
        launcher_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
    ) -> Result<Option<MainMenuOptions>> {
        let options = vec![
            ("output", MainMenuOptions::ShowOutputDeviceMenu.to_str()),
            ("input", MainMenuOptions::ShowInputDeviceMenu.to_str()),
            (
                "output_streams",
                MainMenuOptions::ShowOutputStreamsMenu.to_str(),
            ),
            (
                "input_streams",
                MainMenuOptions::ShowInputStreamsMenu.to_str(),
            ),
            ("settings", MainMenuOptions::ShowSettingsMenu.to_str()),
        ];

        let input = self.get_icon_text(options, icon_type, spaces);

        let menu_output = self.run_launcher(launcher_command, Some(&input), icon_type, None)?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(MainMenuOptions::from_string(&cleaned_output));
        }

        Ok(None)
    }

    pub async fn show_settings_menu(
        &self,
        launcher_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
        back_on_escape: bool,
    ) -> Result<Option<SettingsMenuOptions>> {
        let mut options = vec![(
            "set_sample_rate",
            SettingsMenuOptions::SetSampleRate.to_str(),
        )];

        if !back_on_escape {
            options.push(("back", t!("menus.common.back").to_string().into()));
        }

        let input = self.get_icon_text(options, icon_type, spaces);
        let hint = t!("menus.settings.hint");

        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&hint))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(SettingsMenuOptions::from_string(&cleaned_output));
        }

        Ok(None)
    }

    pub async fn show_sample_rate_menu(
        &self,
        launcher_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
        current_rate: u32,
        back_on_escape: bool,
    ) -> Result<Option<SampleRateMenuOptions>> {
        let common_rates = [44100, 48000, 96000, 192000];
        let mut options = Vec::new();

        for &rate in &common_rates {
            let mut display_name = format!("{:.1} kHz", rate as f32 / 1000.0);

            if rate == current_rate {
                display_name.push_str(&format!(" {}", self.icons.get_icon("default", "generic")));
            }

            options.push(("profile", display_name));
        }

        if !back_on_escape {
            options.push(("back", t!("menus.common.back").to_string()));
        }

        let input = self.get_icon_text(options, icon_type, spaces);
        let hint = t!(
            "menus.sample_rate.hint",
            current_rate = format!("{:.1} kHz", current_rate as f32 / 1000.0)
        );

        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&hint))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(SampleRateMenuOptions::from_string_with_rates(
                &cleaned_output,
                &common_rates,
            ));
        }

        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn show_stream_menu(
        &self,
        launcher_command: &Option<String>,
        streams: &[Node],
        controller: &Controller,
        icon_type: &str,
        spaces: usize,
        is_output: bool,
        back_on_escape: bool,
    ) -> Result<Option<String>> {
        let refresh_text = StreamMenuOptions::RefreshList.to_str();
        let options_start = vec![("refresh", refresh_text.as_ref())];

        let mut input = self.get_icon_text(options_start, icon_type, spaces);

        for stream in streams {
            let display_name = self.format_stream_display_name(stream, controller);

            let volume_str = if stream.volume.muted {
                format!(" [{}]", t!("menus.volume.muted"))
            } else {
                format!(" [{}%]", stream.volume.percent())
            };

            let full_display = format!("{display_name}{volume_str}");
            let formatted = self.format_display_with_icon(
                &full_display,
                &self.icons.get_icon("stream", icon_type),
                icon_type,
                spaces,
            );
            input.push_str(&format!("\n{formatted}"));
        }

        if !back_on_escape {
            let back_text = t!("menus.common.back");
            let back_formatted = self.get_icon_text(vec![("back", back_text)], icon_type, spaces);
            input.push_str(&format!("\n{back_formatted}"));
        }

        let hint = if is_output {
            t!("menus.output_streams.hint")
        } else {
            t!("menus.input_streams.hint")
        };

        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&hint))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(Some(cleaned_output));
        }

        Ok(None)
    }

    pub async fn show_output_device_menu(
        &self,
        launcher_command: &Option<String>,
        nodes: &[Node],
        controller: &Controller,
        icon_type: &str,
        spaces: usize,
        back_on_escape: bool,
    ) -> Result<Option<String>> {
        let refresh_text = OutputDeviceMenuOptions::RefreshList.to_str();
        let options_start = vec![("refresh", refresh_text.as_ref())];

        let mut input = self.get_icon_text(options_start, icon_type, spaces);

        for node in nodes {
            let node_display = self.format_node_display(node, controller, icon_type, spaces);
            input.push_str(&format!("\n{node_display}"));
        }

        if !back_on_escape {
            let back_text = t!("menus.common.back");
            let back_formatted = self.get_icon_text(vec![("back", back_text)], icon_type, spaces);
            input.push_str(&format!("\n{back_formatted}"));
        }

        let hint = t!("menus.output_devices.hint");
        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&hint))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(Some(cleaned_output));
        }

        Ok(None)
    }

    pub async fn show_input_device_menu(
        &self,
        launcher_command: &Option<String>,
        nodes: &[Node],
        controller: &Controller,
        icon_type: &str,
        spaces: usize,
        back_on_escape: bool,
    ) -> Result<Option<String>> {
        let refresh_text = InputDeviceMenuOptions::RefreshList.to_str();
        let options_start = vec![("refresh", refresh_text.as_ref())];

        let mut input = self.get_icon_text(options_start, icon_type, spaces);

        for node in nodes {
            let node_display = self.format_node_display(node, controller, icon_type, spaces);
            input.push_str(&format!("\n{node_display}"));
        }

        if !back_on_escape {
            let back_text = t!("menus.common.back");
            let back_formatted = self.get_icon_text(vec![("back", back_text)], icon_type, spaces);
            input.push_str(&format!("\n{back_formatted}"));
        }

        let hint = t!("menus.input_devices.hint");
        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&hint))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(Some(cleaned_output));
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
        back_on_escape: bool,
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

        if !back_on_escape {
            let back_text = t!("menus.common.back");
            options.push(("back", back_text));
        }

        let input = self.get_icon_text(options, icon_type, spaces);
        let hint = t!("menus.device.hint", device_name = device_name);

        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&hint))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(DeviceMenuOptions::from_string(&cleaned_output));
        }

        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn show_profile_menu(
        &self,
        launcher_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
        device_name: &str,
        profiles: &[Profile],
        current_profile_index: Option<u32>,
        back_on_escape: bool,
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

        if !back_on_escape {
            options.push(("back", t!("menus.common.back").to_string()));
        }

        let input = self.get_icon_text(options, icon_type, spaces);
        let hint = t!("menus.profile.hint", device_name = device_name);

        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&hint))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(ProfileMenuOptions::from_string_with_profiles(
                &cleaned_output,
                profiles,
            ));
        }

        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn show_volume_menu(
        &self,
        launcher_command: &Option<String>,
        icon_type: &str,
        spaces: usize,
        node: &Node,
        is_output_menu: bool,
        last_action: Option<VolumeMenuOptions>,
        device_name: &str,
        volume_display: &str,
        step_percent: u8,
        back_on_escape: bool,
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
                options.push((
                    decrease_key,
                    VolumeMenuOptions::Decrease.to_str(Some(step_percent)),
                ));
                options.push((
                    increase_key,
                    VolumeMenuOptions::Increase.to_str(Some(step_percent)),
                ));
            }
            Some(VolumeMenuOptions::Increase) => {
                options.push((
                    increase_key,
                    VolumeMenuOptions::Increase.to_str(Some(step_percent)),
                ));
                options.push((
                    decrease_key,
                    VolumeMenuOptions::Decrease.to_str(Some(step_percent)),
                ));
            }
            _ => {
                options.push((
                    increase_key,
                    VolumeMenuOptions::Increase.to_str(Some(step_percent)),
                ));
                options.push((
                    decrease_key,
                    VolumeMenuOptions::Decrease.to_str(Some(step_percent)),
                ));
            }
        }

        if node.volume.muted {
            let unmute_key = if is_output_menu {
                "output_unmute"
            } else {
                "input_unmute"
            };
            options.push((unmute_key, VolumeMenuOptions::Unmute.to_str(None)));
        } else {
            let mute_key = if is_output_menu {
                "output_mute"
            } else {
                "input_mute"
            };
            options.push((mute_key, VolumeMenuOptions::Mute.to_str(None)));
        }

        if !back_on_escape {
            let back_text = t!("menus.common.back");
            options.push(("back", back_text));
        }

        let input = self.get_icon_text(options, icon_type, spaces);
        let hint = t!(
            "menus.volume.hint",
            device_name = device_name,
            volume = volume_display
        );

        let menu_output =
            self.run_launcher(launcher_command, Some(&input), icon_type, Some(&hint))?;

        if let Some(output) = menu_output {
            let cleaned_output = self.clean_menu_output(&output, icon_type);
            return Ok(VolumeMenuOptions::from_string(
                &cleaned_output,
                step_percent,
            ));
        }

        Ok(None)
    }
}
