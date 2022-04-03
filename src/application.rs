use gettextrs::gettext;
use log::{debug, info};

use glib::clone;
use gtk4::subclass::prelude::*;
use gtk4::{gdk::Display, gio, glib};
use gtk4::{prelude::*, CssProvider, StyleContext};

use crate::config::{APP_ID, PKGDATADIR, PROFILE, VERSION};
use crate::window::CosmicAppLibraryWindow;

mod imp {
    use super::*;
    use glib::WeakRef;
    use once_cell::sync::OnceCell;

    #[derive(Debug, Default)]
    pub struct CosmicAppLibraryApplication {
        pub window: OnceCell<WeakRef<CosmicAppLibraryWindow>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CosmicAppLibraryApplication {
        const NAME: &'static str = "CosmicAppLibraryApplication";
        type Type = super::CosmicAppLibraryApplication;
        type ParentType = gtk4::Application;
    }

    impl ObjectImpl for CosmicAppLibraryApplication {}

    impl ApplicationImpl for CosmicAppLibraryApplication {
        fn activate(&self, app: &Self::Type) {
            debug!("GtkApplication<CosmicAppLibraryApplication>::activate");
            self.parent_activate(app);

            if let Some(window) = self.window.get() {
                let window = window.upgrade().unwrap();
                window.present();
                return;
            }

            let window = CosmicAppLibraryWindow::new(app);
            self.window
                .set(window.downgrade())
                .expect("Window already set.");

            app.main_window().present();
        }

        fn startup(&self, app: &Self::Type) {
            debug!("GtkApplication<CosmicAppLibraryApplication>::startup");
            self.parent_startup(app);

            // Set icons for shell
            gtk4::Window::set_default_icon_name(APP_ID);

            app.setup_css();
            app.setup_gactions();
            app.setup_accels();
        }
    }

    impl GtkApplicationImpl for CosmicAppLibraryApplication {}
}

glib::wrapper! {
    pub struct CosmicAppLibraryApplication(ObjectSubclass<imp::CosmicAppLibraryApplication>)
        @extends gio::Application, gtk4::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl CosmicAppLibraryApplication {
    pub fn new() -> Self {
        glib::Object::new(&[
            ("application-id", &Some(APP_ID)),
            ("flags", &gio::ApplicationFlags::empty()),
            ("resource-base-path", &Some("/com/System76/AppLibrary/")),
        ])
        .expect("Application initialization failed...")
    }

    fn main_window(&self) -> CosmicAppLibraryWindow {
        self.imp().window.get().unwrap().upgrade().unwrap()
    }

    fn setup_gactions(&self) {
        // Quit
        let action_quit = gio::SimpleAction::new("quit", None);
        action_quit.connect_activate(clone!(@weak self as app => move |_, _| {
            // This is needed to trigger the delete event and saving the window state
            app.main_window().close();
            app.quit();
        }));
        self.add_action(&action_quit);

        // About
        let action_about = gio::SimpleAction::new("about", None);
        action_about.connect_activate(clone!(@weak self as app => move |_, _| {
            app.show_about_dialog();
        }));
        self.add_action(&action_about);
    }

    // Sets up keyboard shortcuts
    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Control>q"]);
    }

    fn setup_css(&self) {
        // Load the css file and add it to the provider
        let provider = CssProvider::new();
        provider.load_from_data(include_bytes!("style.css"));

        // Add the provider to the default screen
        StyleContext::add_provider_for_display(
            &Display::default().expect("Error initializing GTK CSS provider."),
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let theme_provider = CssProvider::new();
        // Add the provider to the default screen
        StyleContext::add_provider_for_display(
            &Display::default().expect("Error initializing GTK CSS provider."),
            &theme_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        glib::MainContext::default().spawn_local(async move {
            if let Err(e) = cosmic_theme::load_cosmic_gtk_theme(theme_provider).await {
                eprintln!("{}", e);
            }
        });
    }

    fn show_about_dialog(&self) {
        let dialog = gtk4::AboutDialog::builder()
            .logo_icon_name(APP_ID)
            // Insert your license of choice here
            // .license_type(gtk4::License::MitX11)
            // Insert your website here
            // .website("https://gitlab.gnome.org/bilelmoussaoui/cosmic-app-library/")
            .version(VERSION)
            .transient_for(&self.main_window())
            .translator_credits(&gettext("translator-credits"))
            .modal(true)
            .authors(vec!["Ashley Wulber".into()])
            .artists(vec!["Ashley Wulber".into()])
            .build();

        dialog.present();
    }

    pub fn run(&self) {
        info!("Cosmic App Library ({})", APP_ID);
        info!("Version: {} ({})", VERSION, PROFILE);
        info!("Datadir: {}", PKGDATADIR);

        ApplicationExtManual::run(self);
    }
}
