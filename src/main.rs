#[rustfmt::skip]
mod config;
mod app;
mod app_group;
mod localize;
mod subscriptions;

use config::APP_ID;
use log::info;

use localize::localize;

use crate::config::{PROFILE, VERSION};

// TODO watch the desktop dirs for changes and update the list of apps on change

fn main() -> cosmic::iced::Result {
    // Initialize logger
    pretty_env_logger::init();
    info!("Cosmic App Library ({})", APP_ID);
    info!("Version: {} ({})", VERSION, PROFILE);

    // Prepare i18n
    localize();

    app::run()
}
