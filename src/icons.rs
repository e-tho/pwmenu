use std::collections::HashMap;

#[derive(Clone)]
pub struct IconDefinition {
    single: String,
    list: String,
}

impl IconDefinition {
    pub fn simple(icon: &str) -> Self {
        Self {
            single: icon.to_string(),
            list: icon.to_string(),
        }
    }

    pub fn with_fallbacks(single: Option<&str>, list: &str) -> Self {
        let single_icon = match single {
            Some(icon) => icon.to_string(),
            None => list.split(',').next().unwrap_or("").trim().to_string(),
        };

        Self {
            single: single_icon,
            list: list.to_string(),
        }
    }
}

#[derive(Clone)]
pub struct Icons {
    generic_icons: HashMap<&'static str, char>,
    font_icons: HashMap<&'static str, char>,
    xdg_icons: HashMap<&'static str, IconDefinition>,
}

impl Icons {
    pub fn new() -> Self {
        let mut generic_icons = HashMap::new();
        let mut font_icons = HashMap::new();
        let mut xdg_icons = HashMap::new();

        // Status Indicators

        generic_icons.insert("default", '\u{23FA}');

        // General

        font_icons.insert("output", '\u{f1120}');
        xdg_icons.insert("output", IconDefinition::simple("audio-speakers-symbolic"));

        font_icons.insert("input", '\u{f036c}');
        xdg_icons.insert(
            "input",
            IconDefinition::simple("audio-input-microphone-symbolic"),
        );

        font_icons.insert("virtual", '\u{f0471}');
        xdg_icons.insert(
            "virtual",
            IconDefinition::simple("applications-multimedia-symbolic"),
        );

        font_icons.insert("monitor", '\u{f1dd}');
        xdg_icons.insert(
            "monitor",
            IconDefinition::with_fallbacks(None, "video-display-symbolic,monitor-symbolic"),
        );

        font_icons.insert("refresh", '\u{f0450}');
        xdg_icons.insert("refresh", IconDefinition::simple("view-refresh-symbolic"));

        font_icons.insert("set_default", '\u{f05e0}');
        xdg_icons.insert(
            "set_default",
            IconDefinition::with_fallbacks(
                None,
                "emblem-default-symbolic,starred-symbolic,star-symbolic",
            ),
        );

        font_icons.insert("switch_profile", '\u{f0ea2}');
        xdg_icons.insert(
            "switch_profile",
            IconDefinition::simple("multimedia-equalizer-symbolic"),
        );

        font_icons.insert("profile", '\u{f0384}');
        xdg_icons.insert(
            "profile",
            IconDefinition::simple("audio-x-generic-symbolic"),
        );

        // Output Controls

        font_icons.insert("output_volume", '\u{f057e}');
        xdg_icons.insert(
            "output_volume",
            IconDefinition::simple("audio-volume-high-symbolic"),
        );

        font_icons.insert("output_volume_up", '\u{f075d}');
        xdg_icons.insert(
            "output_volume_up",
            IconDefinition::with_fallbacks(None, "value-increase-symbolic,list-add-symbolic"),
        );

        font_icons.insert("output_volume_down", '\u{f075e}');
        xdg_icons.insert(
            "output_volume_down",
            IconDefinition::with_fallbacks(None, "value-decrease-symbolic,list-remove-symbolic"),
        );

        font_icons.insert("output_mute", '\u{f0e08}');
        xdg_icons.insert(
            "output_mute",
            IconDefinition::simple("audio-volume-muted-symbolic"),
        );

        font_icons.insert("output_unmute", '\u{f057e}');
        xdg_icons.insert(
            "output_unmute",
            IconDefinition::simple("audio-speakers-symbolic"),
        );

        font_icons.insert("output_volume_low", '\u{f057f}');
        xdg_icons.insert(
            "output_volume_low",
            IconDefinition::simple("audio-volume-low-symbolic"),
        );

        font_icons.insert("output_volume_medium", '\u{f0580}');
        xdg_icons.insert(
            "output_volume_medium",
            IconDefinition::simple("audio-volume-medium-symbolic"),
        );

        font_icons.insert("output_volume_high", '\u{f057e}');
        xdg_icons.insert(
            "output_volume_high",
            IconDefinition::simple("audio-volume-high-symbolic"),
        );

        // Input Controls

        font_icons.insert("input_volume", '\u{f057e}');
        xdg_icons.insert(
            "input_volume",
            IconDefinition::simple("microphone-sensitivity-high-symbolic"),
        );

        font_icons.insert("input_volume_up", '\u{f08b4}');
        xdg_icons.insert(
            "input_volume_up",
            IconDefinition::with_fallbacks(
                None,
                "value-increase-symbolic,list-add-symbolic,add-symbolic",
            ),
        );

        font_icons.insert("input_volume_down", '\u{f08b3}');
        xdg_icons.insert(
            "input_volume_down",
            IconDefinition::with_fallbacks(
                None,
                "value-decrease-symbolic,list-remove-symbolic,remove-symbolic",
            ),
        );

        font_icons.insert("input_mute", '\u{f036d}');
        xdg_icons.insert(
            "input_mute",
            IconDefinition::simple("microphone-sensitivity-muted-symbolic"),
        );

        font_icons.insert("input_unmute", '\u{f036c}');
        xdg_icons.insert(
            "input_unmute",
            IconDefinition::simple("audio-input-microphone-symbolic"),
        );

        font_icons.insert("input_volume_low", '\u{f057f}');
        xdg_icons.insert(
            "input_volume_low",
            IconDefinition::simple("microphone-sensitivity-low-symbolic"),
        );

        font_icons.insert("input_volume_medium", '\u{f0580}');
        xdg_icons.insert(
            "input_volume_medium",
            IconDefinition::simple("microphone-sensitivity-medium-symbolic"),
        );

        font_icons.insert("input_volume_high", '\u{f057e}');
        xdg_icons.insert(
            "input_volume_high",
            IconDefinition::simple("microphone-sensitivity-high-symbolic"),
        );

        // Device types

        font_icons.insert("headphones", '\u{f02cb}');
        xdg_icons.insert(
            "headphones",
            IconDefinition::simple("audio-headphones-symbolic"),
        );

        font_icons.insert("speaker", '\u{f04c3}');
        xdg_icons.insert("speaker", IconDefinition::simple("audio-speakers-symbolic"));

        font_icons.insert("hdmi", '\u{f0841}');
        xdg_icons.insert(
            "hdmi",
            IconDefinition::with_fallbacks(
                None,
                "video-display-symbolic,monitor-symbolic,display-symbolic",
            ),
        );

        font_icons.insert("microphone", '\u{f036c}');
        xdg_icons.insert(
            "microphone",
            IconDefinition::simple("audio-input-microphone-symbolic"),
        );

        font_icons.insert("analog", '\u{f1543}');
        xdg_icons.insert("analog", IconDefinition::simple("audio-card-symbolic"));

        font_icons.insert("digital", '\u{f0697}');
        xdg_icons.insert("digital", IconDefinition::simple("computer-symbolic"));

        font_icons.insert("bluetooth", '\u{f00af}');
        xdg_icons.insert(
            "bluetooth",
            IconDefinition::with_fallbacks(
                None,
                "bluetooth-symbolic,network-bluetooth-symbolic,bluetooth-active-symbolic",
            ),
        );

        font_icons.insert("usb", '\u{f11f0}');
        xdg_icons.insert(
            "usb",
            IconDefinition::with_fallbacks(
                None,
                "media-removable-symbolic,drive-removable-media-usb-symbolic",
            ),
        );

        Icons {
            font_icons,
            xdg_icons,
            generic_icons,
        }
    }

    pub fn get_icon(&self, key: &str, icon_type: &str) -> String {
        match icon_type {
            "font" => self
                .font_icons
                .get(key)
                .map(|&icon| icon.to_string())
                .unwrap_or_default(),
            "xdg" => self
                .xdg_icons
                .get(key)
                .map(|icon_definition| icon_definition.list.clone())
                .unwrap_or_default(),
            "generic" => self
                .generic_icons
                .get(key)
                .map(|&icon| icon.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    pub fn get_xdg_icon(&self, key: &str) -> String {
        self.xdg_icons
            .get(key)
            .map(|icon_def| icon_def.single.clone())
            .unwrap_or_default()
    }

    pub fn get_icon_text<T>(&self, items: Vec<(&str, T)>, icon_type: &str, spaces: usize) -> String
    where
        T: AsRef<str>,
    {
        items
            .into_iter()
            .map(|(icon_key, text)| {
                let icon = self.get_icon(icon_key, icon_type);
                let text = text.as_ref();
                match icon_type {
                    "font" => format!("{}{}{}", icon, " ".repeat(spaces), text),
                    "xdg" => format!("{}\0icon\x1f{}", text, icon),
                    _ => text.to_string(),
                }
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn format_with_spacing(icon: char, spaces: usize, before: bool) -> String {
        if before {
            format!("{}{}", " ".repeat(spaces), icon)
        } else {
            format!("{}{}", icon, " ".repeat(spaces))
        }
    }

    pub fn format_display_with_icon(
        &self,
        name: &str,
        icon: &str,
        icon_type: &str,
        spaces: usize,
    ) -> String {
        match icon_type {
            "xdg" => format!("{}\0icon\x1f{}", name, icon),
            "font" | "generic" => format!("{}{}{}", icon, " ".repeat(spaces), name),
            _ => name.to_string(),
        }
    }

    pub fn get_device_icon(&self, device_type: &str, icon_type: &str) -> String {
        let icon_key = match device_type.to_lowercase().as_str() {
            s if s.contains("headphone") => "headphones",
            s if s.contains("speaker") => "speaker",
            s if s.contains("hdmi") => "hdmi",
            s if s.contains("mic") => "microphone",
            s if s.contains("analog") => "analog",
            s if s.contains("digital") => "digital",
            s if s.contains("bluetooth") => "bluetooth",
            s if s.contains("usb") => "usb",
            _ => "output",
        };

        self.get_icon(icon_key, icon_type)
    }
}

impl Default for Icons {
    fn default() -> Self {
        Self::new()
    }
}
