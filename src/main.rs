use anyhow::{anyhow, Result};
use clap::{builder::EnumValueParser, Arg, Command};
use pwmenu::{app::App, icons::Icons, launcher::LauncherType, menu::Menu};
use rust_i18n::{available_locales, i18n, set_locale};
use std::{env, sync::Arc};
use sys_locale::get_locale;
use tokio::sync::mpsc::unbounded_channel;

i18n!("locales");

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
                .conflicts_with("menu")
                .help("Launcher to use (replaces deprecated --menu)"),
        )
        .arg(
            Arg::new("menu") // Deprecated
                .short('m')
                .long("menu")
                .takes_value(true)
                .value_parser(EnumValueParser::<LauncherType>::new())
                .hide(true)
                .help("DEPRECATED: use --launcher instead"),
        )
        .arg(
            Arg::new("launcher_command")
                .long("launcher-command")
                .takes_value(true)
                .required_if_eq("launcher", "custom")
                .conflicts_with("menu_command")
                .help("Launcher command to use when --launcher is set to custom"),
        )
        .arg(
            Arg::new("menu_command") // Deprecated
                .long("menu-command")
                .takes_value(true)
                .required_if_eq("menu", "custom")
                .hide(true)
                .help("DEPRECATED: use --launcher-command instead"),
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
        .get_matches();

    let launcher_type: LauncherType = if matches.contains_id("launcher") {
        matches
            .get_one::<LauncherType>("launcher")
            .cloned()
            .unwrap()
    } else if matches.contains_id("menu") {
        eprintln!("WARNING: --menu flag is deprecated. Please use --launcher instead.");
        matches.get_one::<LauncherType>("menu").cloned().unwrap()
    } else {
        LauncherType::Dmenu
    };

    let command_str = if matches.contains_id("launcher_command") {
        matches.get_one::<String>("launcher_command").cloned()
    } else if matches.contains_id("menu_command") {
        eprintln!(
            "WARNING: --menu-command flag is deprecated. Please use --launcher-command instead."
        );
        matches.get_one::<String>("menu_command").cloned()
    } else {
        None
    };

    let icon_type = matches.get_one::<String>("icon").cloned().unwrap();

    let icons = Arc::new(Icons::new());
    let menu = Menu::new(launcher_type, icons.clone());

    let spaces = matches
        .get_one::<String>("spaces")
        .and_then(|s| s.parse::<usize>().ok())
        .ok_or_else(|| anyhow!("Invalid value for --spaces. Must be a positive integer."))?;

    let (log_sender, mut log_receiver) = unbounded_channel::<String>();

    tokio::spawn(async move {
        while let Some(log) = log_receiver.recv().await {
            println!("LOG: {}", log);
        }
    });

    run_app_loop(&menu, &command_str, &icon_type, spaces, log_sender, icons).await?;

    Ok(())
}

async fn run_app_loop(
    menu: &Menu,
    command_str: &Option<String>,
    icon_type: &str,
    spaces: usize,
    log_sender: tokio::sync::mpsc::UnboundedSender<String>,
    icons: Arc<Icons>,
) -> Result<()> {
    let mut app = App::new(menu.clone(), log_sender.clone(), icons.clone()).await?;

    loop {
        match app.run(menu, command_str, icon_type, spaces).await {
            Ok(_) => {
                if !app.reset_mode {
                    break;
                }
            }
            Err(err) => {
                eprintln!("Error during app execution: {:?}", err);

                if !app.reset_mode {
                    return Err(anyhow!("Fatal error in application: {}", err));
                }
            }
        }

        if app.reset_mode {
            app = App::new(menu.clone(), log_sender.clone(), icons.clone()).await?;
            app.reset_mode = false;
        }
    }

    Ok(())
}
