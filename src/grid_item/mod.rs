// SPDX-License-Identifier: GPL-3.0-only
use cascade::cascade;
use gettextrs::gettext;
use gtk4::{
    gdk::{self, ContentProvider},
    gio::File,
    glib,
    pango::EllipsizeMode,
    prelude::*,
    subclass::prelude::*,
    traits::WidgetExt,
    Align, Button, DragSource, IconTheme, Image, Label, Orientation,
};
use std::path::{Path, PathBuf};

use crate::app_group::BoxedAppGroupType;
use crate::{app_group::AppGroup, desktop_entry_data::DesktopEntryData};

mod imp;

glib::wrapper! {
pub struct GridItem(ObjectSubclass<imp::GridItem>)
        @extends gtk4::Widget, gtk4::Box,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl Default for GridItem {
    fn default() -> Self {
        Self::new()
    }
}

impl GridItem {
    pub fn new() -> Self {
        let self_ = glib::Object::new(&[]).expect("Failed to create GridItem");
        let imp = imp::GridItem::from_instance(&self_);

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
        let imp = imp::GridItem::from_instance(self);
        imp.icon_theme.set(icon_theme).unwrap();
    }

    pub fn set_desktop_entry_data(&self, desktop_entry_data: &DesktopEntryData) {
        let self_ = imp::GridItem::from_instance(self);
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
            drag_controller.connect_drag_begin(glib::clone!(@weak icon, => move |_self, drag| {
                drag.set_selected_action(gdk::DragAction::MOVE);
                _self.set_icon(Some(&icon), 32, 32);
            }));
        };
    }

    pub fn set_group_info(&self, app_group: AppGroup) {
        // if data type set name and icon to values in data
        let imp = imp::GridItem::from_instance(self);
        match app_group.property::<BoxedAppGroupType>("inner") {
            BoxedAppGroupType::Group(data) => {
                imp.name.borrow().set_text(&data.name);
                imp.image.borrow().set_from_icon_name(Some(&data.icon));
            }
            BoxedAppGroupType::NewGroup(popover_active) => {
                // else must be add group
                imp.name.borrow().set_text(&gettext("New Group"));
                imp.image.borrow().set_from_icon_name(Some("folder-new"));

                let popover_menu = gtk4::Box::builder()
                    .spacing(12)
                    .hexpand(true)
                    .orientation(gtk4::Orientation::Vertical)
                    .margin_top(12)
                    .margin_bottom(12)
                    .margin_end(12)
                    .margin_start(12)
                    .build();
                // build menu
                let dialog_entry = cascade! {
                    gtk4::Entry::new();
                    ..add_css_class("background-component");
                    ..add_css_class("border-radius-medium");
                };
                let label = cascade! {
                    Label::new(Some(&gettext("Name")));
                    ..set_justify(gtk4::Justification::Left);
                    ..set_xalign(0.0);
                };
                popover_menu.append(&label);
                popover_menu.append(&dialog_entry);
                let btn_container = cascade! {
                    gtk4::Box::new(Orientation::Horizontal, 8);
                    ..add_css_class("background");
                };
                let ok_btn = cascade! {
                    Button::with_label(&gettext("Ok"));
                    ..add_css_class("suggested-action");
                    ..add_css_class("border-radius-medium");
                };
                let cancel_btn = cascade! {
                    Button::with_label(&gettext("Cancel"));
                    ..add_css_class("destructive-action");
                    ..add_css_class("border-radius-medium");
                };
                btn_container.append(&ok_btn);
                btn_container.append(&cancel_btn);
                popover_menu.append(&btn_container);
                let popover = cascade! {
                    gtk4::Popover::new();
                    ..set_autohide(true);
                    ..set_child(Some(&popover_menu));
                };
                self.append(&popover);

                popover.connect_closed(
                    glib::clone!(@weak self as self_, @weak dialog_entry => move |_| {
                        dialog_entry.set_text("");
                        self_.emit_by_name::<()>("popover-closed", &[]);
                    }),
                );
                ok_btn.connect_clicked(
                    glib::clone!(@weak self as self_, @weak dialog_entry, @weak popover => move |_| {
                        let new_name = dialog_entry.text().to_string();
                        popover.popdown();
                        glib::idle_add_local_once(glib::clone!(@weak self_ => move || {
                            self_.emit_by_name::<()>("new-group", &[&new_name]);
                        }));
                    }),
                );
                cancel_btn.connect_clicked(glib::clone!(@weak popover => move |_| {
                    popover.popdown();
                }));
                if popover_active {
                    popover.popup();
                }

                imp.popover.replace(Some(popover));
            }
        }
    }

    pub fn set_index(&self, index: u32) {
        imp::GridItem::from_instance(self).index.set(index);
    }

    pub fn popup(&self) {
        let imp = imp::GridItem::from_instance(self);
        if let Some(popover) = imp.popover.borrow().as_ref() {
            popover.popup();
        }
    }
}
