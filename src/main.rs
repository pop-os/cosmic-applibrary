mod application;
#[rustfmt::skip]
mod config;
mod app_grid;
mod app_group;
mod desktop_entry_data;
mod grid_item;
mod group_grid;
mod localize;
mod utils;
mod window;
mod window_inner;

use gtk4::{gio, glib};

use self::application::CosmicAppLibraryApplication;
use self::config::RESOURCES_FILE;

pub fn localize() {
    let localizer = crate::localize::localizer();
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    if let Err(error) = localizer.select(&requested_languages) {
        eprintln!(
            "Error while loading language for pop-desktop-widget {}",
            error
        );
    }
}

fn main() {
    // Initialize logger
    pretty_env_logger::init();
    glib::set_application_name("Cosmic App Library");

    localize();

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    let app = CosmicAppLibraryApplication::new();
    app.run();
}
