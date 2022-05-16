use crate::{application::CosmicAppLibraryApplication, fl, window_inner::AppLibraryWindowInner};
use cascade::cascade;
use gdk4_x11::X11Display;
use gtk4::{
    gio,
    glib::{self, Object},
    prelude::*,
    subclass::prelude::*,
};
use libcosmic::x;

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
        self_.setup_shortcuts();

        self_
    }

    fn setup_shortcuts(&self) {
        let window = self.clone().upcast::<gtk4::Window>();
        let action_quit = gio::SimpleAction::new("quit", None);
        action_quit.connect_activate(glib::clone!(@weak window => move |_, _| {
            window.close();
            window.application().map(|a| a.quit());
            std::process::exit(0);
        }));
        self.add_action(&action_quit);
    }

    fn setup_callbacks(&self) {
        // Get state
        let window = self.clone().upcast::<gtk4::Window>();

        window.connect_realize(move |window| {
            if let Some((display, surface)) = x::get_window_x11(window) {
                // ignore all x11 errors...
                let xdisplay = display
                    .clone()
                    .downcast::<X11Display>()
                    .expect("Failed to downgrade X11 Display.");
                xdisplay.error_trap_push();
                unsafe {
                    x::change_property(
                        &display,
                        &surface,
                        "_NET_WM_WINDOW_TYPE",
                        x::PropMode::Replace,
                        &[x::Atom::new(&display, "_NET_WM_WINDOW_TYPE_DIALOG").unwrap()],
                    );
                }
                let resize = glib::clone!(@weak window => move || {
                    let height = window.height();
                    let width = window.width();

                    if let Some((display, _surface)) = x::get_window_x11(&window) {
                        let geom = display
                            .primary_monitor().geometry();
                        let monitor_x = geom.x();
                        let monitor_y = geom.y();
                        let monitor_width = geom.width();
                        let monitor_height = geom.height();
                        // dbg!(monitor_width);
                        // dbg!(monitor_height);
                        // dbg!(width);
                        // dbg!(height);
                        unsafe { x::set_position(&display, &surface,
                            monitor_x + monitor_width / 2 - width / 2,
                                                 monitor_y + monitor_height / 2 - height / 2)};
                    }
                });
                let s = window.surface();
                let resize_height = resize.clone();
                s.connect_height_notify(move |_s| {
                    glib::source::idle_add_local_once(resize_height.clone());
                });
                let resize_width = resize.clone();
                s.connect_width_notify(move |_s| {
                    glib::source::idle_add_local_once(resize_width.clone());
                });
                s.connect_scale_factor_notify(move |_s| {
                    glib::source::idle_add_local_once(resize.clone());
                });
            } else {
                eprintln!("failed to get X11 window");
            }
        });

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
                win.close();
                win.application().map(|a| a.quit());
                std::process::exit(0);
            }
        }));
    }
}
