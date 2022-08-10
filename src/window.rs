use crate::{application::CosmicAppLibraryApplication, fl, window_inner::AppLibraryWindowInner};
use cascade::cascade;
use gtk4::{
    gio,
    glib::{self, Object},
    prelude::*,
    subclass::prelude::*,
};

mod imp {
    use super::*;
    // SPDX-License-Identifier: MPL-2.0-only
    use crate::window_inner::AppLibraryWindowInner;
    use gtk4::glib;
    use once_cell::sync::OnceCell;

    // Object holding the state
    #[derive(Default)]

    pub struct CosmicAppLibraryWindow {
        pub(super) inner: OnceCell<AppLibraryWindowInner>,
    }

    // The central trait for subclassing a GObject
    #[glib::object_subclass]
    impl ObjectSubclass for CosmicAppLibraryWindow {
        // `NAME` needs to match `class` attribute of template
        const NAME: &'static str = "CosmicAppLibraryWindow";
        type Type = super::CosmicAppLibraryWindow;
        type ParentType = gtk4::ApplicationWindow;
    }

    // Trait shared by all GObjects
    impl ObjectImpl for CosmicAppLibraryWindow {}

    // Trait shared by all widgets
    impl WidgetImpl for CosmicAppLibraryWindow {}

    // Trait shared by all windows
    impl WindowImpl for CosmicAppLibraryWindow {}

    // Trait shared by all application
    impl ApplicationWindowImpl for CosmicAppLibraryWindow {}
}

glib::wrapper! {
    pub struct CosmicAppLibraryWindow(ObjectSubclass<imp::CosmicAppLibraryWindow>)
        @extends gtk4::ApplicationWindow, gtk4::Window, gtk4::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl CosmicAppLibraryWindow {
    pub fn new(app: &CosmicAppLibraryApplication) -> Self {
        let self_: Self = Object::new(&[("application", app)])
            .expect("Failed to create `CosmicAppLibraryWindow`.");
        let imp = imp::CosmicAppLibraryWindow::from_instance(&self_);

        cascade! {
            &self_;
            ..set_width_request(1200);
            ..set_title(Some(&fl!("cosmic-app-library")));
            ..set_decorated(false);
            ..add_css_class("root_window");
            ..add_css_class("padding-medium");
            ..add_css_class("border-radius-medium");
        };
        let app_library = AppLibraryWindowInner::new();
        self_.set_child(Some(&app_library));
        imp.inner.set(app_library).unwrap();

        self_.setup_callbacks();
        // self_.setup_shortcuts();

        self_
    }

    fn setup_callbacks(&self) {
        // Get state
        let window = self.clone().upcast::<gtk4::Window>();

        let imp = imp::CosmicAppLibraryWindow::from_instance(&self);
        let inner = imp.inner.get().unwrap();
        window.connect_is_active_notify(glib::clone!(@weak inner => move |win| {
            let app = win
                .application()
                .expect("could not get application from window");
            let active_window = app
                .active_window()
                .expect("no active window available, closing app library.");
            if win == &active_window && !win.is_active() && !inner.is_popup_active() {
                inner.clear();
            }
        }));
    }
}
