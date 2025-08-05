use anyhow::{anyhow, Result};
use clap::{builder::EnumValueParser, Arg, Command};
use pwmenu::{app::App, icons::Icons, launcher::LauncherType, menu::Menu};
use rust_i18n::{available_locales, i18n, set_locale};
use std::{env, sync::Arc};
use sys_locale::get_locale;
use tokio::sync::mpsc::unbounded_channel;

i18n!("locales");

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
    let locale = get_locale().unwrap_or_else(|| {
        eprintln!("Locale not detected, defaulting to 'en-US'.");
        String::from("en-US")
    });
    if available_locales!().iter().any(|&x| x == locale) {
        set_locale(&locale);
    } else {
        set_locale("en");
    }

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
                .possible_values(["outputs", "inputs"])
                .help("Start in the specified root menu"),
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

    let (log_sender, mut log_receiver) = unbounded_channel::<String>();

    tokio::spawn(async move {
        while let Some(log) = log_receiver.recv().await {
            println!("LOG: {log}");
        }
    });

    run_app_loop(
        &menu,
        &command_str,
        &icon_type,
        spaces,
        log_sender,
        icons,
        root_menu,
    )
    .await?;

    Ok(())
}

async fn run_app_loop(
    menu: &Menu,
    command_str: &Option<String>,
    icon_type: &str,
    spaces: usize,
    log_sender: tokio::sync::mpsc::UnboundedSender<String>,
    icons: Arc<Icons>,
    root_menu: Option<String>,
) -> Result<()> {
    let mut app = App::new(menu.clone(), log_sender.clone(), icons.clone()).await?;

    let result = if let Some(ref menu_name) = root_menu {
        app.wait_for_initialization().await?;
        match menu_name.as_str() {
            "outputs" => {
                app.run_output_menu(menu, command_str, icon_type, spaces)
                    .await
            }
            "inputs" => {
                app.run_input_menu(menu, command_str, icon_type, spaces)
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
