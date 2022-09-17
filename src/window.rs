use crate::{application::CosmicAppLibraryApplication, fl, window_inner::AppLibraryWindowInner};
use cascade::cascade;
use gtk4::{
    gio,
    glib::{self, Object},
    prelude::*,
    subclass::prelude::*, gdk,
};
use zbus::Connection;

mod imp {
    use super::*;
    // SPDX-License-Identifier: MPL-2.0-only
    use crate::window_inner::AppLibraryWindowInner;
    use gtk4::glib;
    use once_cell::sync::OnceCell;
    use std::rc::Rc;

    // Object holding the state
    #[derive(Default)]

    pub struct CosmicAppLibraryWindow {
        pub(super) inner: OnceCell<AppLibraryWindowInner>,
        pub dbus_conn: Rc<OnceCell<Connection>>,
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
        glib::MainContext::default().spawn_local(glib::clone!(@weak self_ => async move {
            let connection = Connection::session().await.unwrap();
            let imp = self_.imp();
            imp.dbus_conn.set(connection).unwrap();
        }));
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
        self.connect_realize(|window| {
            window.surface().downcast::<gdk::Toplevel>().unwrap().connect_state_notify(glib::clone!( @weak window => move |toplevel| {
                let state = toplevel.state();
                if state.contains(gdk::ToplevelState::FOCUSED) {
                    window.show();
                } else {
                    window.clear()
                }
            }));
        });
        self.connect_is_active_notify(|win| {
            if !win.is_active() {
                win.clear();
            }
        });

        let action_quit = gio::SimpleAction::new("quit", None);
        // TODO clear state instead of closing
        let conn = &self.imp().dbus_conn;
        action_quit.connect_activate(glib::clone!(@weak conn  => move |_, _| {
            glib::MainContext::default().spawn_local(glib::clone!(@weak conn => async move {
                if let Some(conn) = conn.get() {
                    let _ = conn.call_method(Some("com.system76.CosmicAppletHost"), "/com/system76/CosmicAppletHost", Some("com.system76.CosmicAppletHost"), "Hide", &("com.system76.CosmicAppLibrary")).await;
                }
            }));
        }));
        self.add_action(&action_quit);
    }

    pub fn clear(&self) {
        self.hide();
        if let Some(inner) = self.imp().inner.get() {
            inner.clear();
        }
        let conn = &self.imp().dbus_conn;
        glib::MainContext::default().spawn_local(glib::clone!(@weak conn => async move {
            if let Some(conn) = conn.get() {
                let _ = conn.call_method(Some("com.system76.CosmicAppletHost"), "/com/system76/CosmicAppletHost", Some("com.system76.CosmicAppletHost"), "Hide", &("com.system76.CosmicAppLibrary")).await;
            }
        }));
        self.show();
    }
}
