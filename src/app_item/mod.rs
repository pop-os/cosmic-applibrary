// SPDX-License-Identifier: MPL-2.0-only
use crate::{
    desktop_entry_data::DesktopEntryData, utils,
};
use cascade::cascade;
use gtk4::{
    gdk::{self, ContentProvider, Display},
    gio::{DesktopAppInfo, File, Icon},
    glib,
    pango::EllipsizeMode,
    prelude::*,
    subclass::prelude::*,
    traits::WidgetExt,
    Align, DragSource, IconTheme, Image, Label, Orientation,
};
use std::path::{Path, PathBuf};

mod imp;

glib::wrapper! {
pub struct AppItem(ObjectSubclass<imp::AppItem>)
        @extends gtk4::Widget, gtk4::Box,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl Default for AppItem {
    fn default() -> Self {
        Self::new()
    }
}

impl AppItem {
    pub fn new() -> Self {
        let self_ = glib::Object::new(&[]).expect("Failed to create GridItem");
        let imp = imp::AppItem::from_instance(&self_);

        cascade! {
            &self_;
            ..set_orientation(Orientation::Vertical);
            ..set_halign(Align::Center);
            ..set_hexpand(true);
            ..set_margin_top(4);
            ..set_margin_bottom(4);
            ..set_margin_end(4);
            ..set_margin_start(4);
        };

        let image = cascade! {
            Image::new();
            ..set_margin_top(4);
            ..set_margin_bottom(4);
            ..set_pixel_size(64);
        };
        self_.append(&image);

        let name = cascade! {
            Label::new(None);
            ..set_halign(Align::Center);
            ..set_hexpand(true);
            ..set_ellipsize(EllipsizeMode::End);
            ..add_css_class("title-5");
        };
        self_.append(&name);

        imp.name.replace(name);
        imp.image.replace(image);
        self_
    }

    pub fn set_icon_theme(&self, icon_theme: IconTheme) {
        let imp = imp::AppItem::from_instance(self);
        imp.icon_theme.set(icon_theme).unwrap();
    }

    pub fn set_desktop_entry_data(&self, desktop_entry_data: &DesktopEntryData) {
        let self_ = imp::AppItem::from_instance(self);
        self_.name.borrow().set_text(&desktop_entry_data.name());

        let drag_controller = DragSource::builder()
            .name("application library drag source")
            .actions(gdk::DragAction::COPY)
            // .content()
            .build();
        self.add_controller(&drag_controller);
        let file = File::for_path(desktop_entry_data.path());
        let provider = ContentProvider::for_value(&file.to_value());
        drag_controller.set_content(Some(&provider));

        if utils::in_flatpak() {
            // TODO set text direction, scale and theme for icons
            let icon_theme = self_.icon_theme.get().unwrap();
            let icon_name = desktop_entry_data.icon().unwrap_or_default();
            let mut p = PathBuf::from(&icon_name);
            if p.has_root() {
                if p.starts_with("/usr") {
                    let stripped_path = p.strip_prefix("/").unwrap_or(&p);
                    p = Path::new("/var/run/host").join(stripped_path);
                }
                self_.image.borrow().set_from_file(Some(p));
            } else {
                let icon_size = icon_theme
                    .icon_sizes(&icon_name)
                    .into_iter()
                    .max()
                    .unwrap_or(1);
                let icon = self_.icon_theme.get().unwrap().lookup_icon(
                    &icon_name,
                    &[],
                    icon_size,
                    1,
                    gtk4::TextDirection::Ltr,
                    gtk4::IconLookupFlags::PRELOAD,
                );
                self_.image.borrow().set_paintable(Some(&icon));
                drag_controller.connect_drag_begin(
                    glib::clone!(@weak icon, => move |_self, drag| {
                        drag.set_selected_action(gdk::DragAction::MOVE);
                        _self.set_icon(Some(&icon), 32, 32);
                    }),
                );
            };
        } else if let Some(app_info) = DesktopAppInfo::from_filename(desktop_entry_data.path()) {
            let icon = app_info
                .icon()
                .unwrap_or(Icon::for_string("image-missing").expect("Failed to set default icon"));
            self_.image.borrow().set_from_gicon(&icon);
            drag_controller.connect_drag_begin(glib::clone!(@weak icon, => move |_self, drag| {
                drag.set_selected_action(gdk::DragAction::MOVE);
                // set drag source icon if possible...
                // gio Icon is not easily converted to a Paintable, but this seems to be the correct method
                if let Some(default_display) = &Display::default() {
                    let icon_theme = IconTheme::for_display(default_display);
                    let paintable_icon = icon_theme.lookup_by_gicon(
                        &icon,
                        64,
                        1,
                        gtk4::TextDirection::None,
                        gtk4::IconLookupFlags::empty(),
                    );
                    _self.set_icon(Some(&paintable_icon), 32, 32);
                }
            }));
        }
    }

    pub fn set_index(&self, index: u32) {
        imp::AppItem::from_instance(self).index.set(index);
    }


}
