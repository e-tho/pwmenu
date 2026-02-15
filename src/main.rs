use anyhow::{anyhow, Result};
use clap::{builder::EnumValueParser, value_parser, Arg, Command};
use pwmenu::{app::App, icons::Icons, launcher::LauncherType, menu::Menu};
use rust_i18n::{i18n, set_locale};
use std::{env, sync::Arc};
use sys_locale::get_locale;

i18n!("locales", fallback = "en");

fn validate_launcher_command(command: &str) -> Result<String, String> {
    if command.contains("{placeholder}") {
        eprintln!("WARNING: {{placeholder}} is deprecated. Use {{hint}} instead.");
    }
    if command.contains("{prompt}") {
        eprintln!("WARNING: {{prompt}} is deprecated. Use {{hint}} instead.");
    }

    Ok(command.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let locale = get_locale().unwrap_or_else(|| String::from("en"));
    set_locale(&locale);

    let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::new("launcher")
                .short('l')
                .long("launcher")
                .required(true)
                .takes_value(true)
                .value_parser(EnumValueParser::<LauncherType>::new())
                .help("Launcher to use"),
        )
        .arg(
            Arg::new("launcher_command")
                .long("launcher-command")
                .takes_value(true)
                .required_if_eq("launcher", "custom")
                .value_parser(validate_launcher_command)
                .help("Launcher command to use when --launcher is set to custom"),
        )
        .arg(
            Arg::new("icon")
                .short('i')
                .long("icon")
                .takes_value(true)
                .possible_values(["font", "xdg"])
                .default_value("font")
                .help("Choose the type of icons to use"),
        )
        .arg(
            Arg::new("spaces")
                .short('s')
                .long("spaces")
                .takes_value(true)
                .default_value("1")
                .help("Number of spaces between icon and text when using font icons"),
        )
        .arg(
            Arg::new("menu")
                .short('m')
                .long("menu")
                .takes_value(true)
                .possible_values([
                    "output-devices",
                    "input-devices",
                    "output-streams",
                    "input-streams",
                ])
                .help("Start in the specified root menu"),
        )
        .arg(
            Arg::new("volume_step")
                .long("volume-step")
                .takes_value(true)
                .value_parser(value_parser!(u8).range(1..=25))
                .default_value("5")
                .help("Volume adjustment step as percentage (1-25)"),
        )
        .arg(
            Arg::new("back_on_escape")
                .long("back-on-escape")
                .takes_value(false)
                .help("Return to previous menu on escape instead of exiting"),
        )
        .get_matches();

    let launcher_type: LauncherType = matches
        .get_one::<LauncherType>("launcher")
        .cloned()
        .unwrap();

    let command_str = matches.get_one::<String>("launcher_command").cloned();

    let icon_type = matches.get_one::<String>("icon").cloned().unwrap();

    let root_menu = matches.get_one::<String>("menu").cloned();

    let icons = Arc::new(Icons::new());
    let menu = Menu::new(launcher_type, icons.clone());

    let spaces = matches
        .get_one::<String>("spaces")
        .and_then(|s| s.parse::<usize>().ok())
        .ok_or_else(|| anyhow!("Invalid value for --spaces. Must be a positive integer."))?;

    let volume_step = matches.get_one::<u8>("volume_step").copied().unwrap() as f32 / 100.0;

    let back_on_escape = matches.contains_id("back_on_escape");

    run_app_loop(
        &menu,
        &command_str,
        &icon_type,
        spaces,
        icons,
        root_menu,
        volume_step,
        back_on_escape,
    )
    .await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_app_loop(
    menu: &Menu,
    command_str: &Option<String>,
    icon_type: &str,
    spaces: usize,
    icons: Arc<Icons>,
    root_menu: Option<String>,
    volume_step: f32,
    back_on_escape: bool,
) -> Result<()> {
    let mut app = App::new(menu.clone(), icons.clone(), volume_step, back_on_escape).await?;

    let result = if let Some(ref menu_name) = root_menu {
        app.wait_for_initialization().await?;
        match menu_name.as_str() {
            "output-devices" => {
                app.run_output_device_menu(menu, command_str, icon_type, spaces)
                    .await
            }
            "input-devices" => {
                app.run_input_device_menu(menu, command_str, icon_type, spaces)
                    .await
            }
            "output-streams" => {
                app.run_output_streams_menu(menu, command_str, icon_type, spaces)
                    .await
            }
            "input-streams" => {
                app.run_input_streams_menu(menu, command_str, icon_type, spaces)
                    .await
            }
            _ => Err(anyhow!("Invalid menu value: {menu_name}")),
        }
    } else {
        app.run(menu, command_str, icon_type, spaces).await
    };

    if let Err(err) = result {
        return Err(anyhow!("Fatal error in application: {err}"));
    }

    Ok(())
}
