mod application;
#[rustfmt::skip]
mod config;
mod app_grid;
mod app_group;
mod desktop_entry_data;
mod grid_item;
mod group_grid;
mod utils;
mod window;
mod window_inner;

use gettextrs::{gettext, LocaleCategory};
use gtk4::{gio, glib};

use self::application::CosmicAppLibraryApplication;
use self::config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};

fn main() {
    // Initialize logger
    pretty_env_logger::init();

    // Prepare i18n
    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name(&gettext("Cosmic App Library"));

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    let app = CosmicAppLibraryApplication::new();
    app.run();
}
