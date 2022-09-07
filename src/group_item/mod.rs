mod imp;

use cascade::cascade;
use glib::Object;
use gtk4::{glib, prelude::*, subclass::prelude::*, ToggleButton, Orientation, Align, Image, Label, pango::EllipsizeMode, Button};
use relm4_macros::view;

use crate::{app_group::{AppGroup, BoxedAppGroupType}, fl};

glib::wrapper! {
    pub struct GroupItem(ObjectSubclass<imp::GroupItem>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Actionable, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl GroupItem {
    pub fn new(toggle_for_group: &ToggleButton) -> Self {
        let self_ = Object::new(&[]).expect("Failed to create `GroupItem`.");
        let imp = imp::GroupItem::from_instance(&self_);

        cascade! {
            &self_;
            ..set_orientation(Orientation::Vertical);
        };

        view! {
            toggle_button = ToggleButton {
                // set_halign: Align::Center,
                set_hexpand: true,
                #[wrap(Some)]
                set_child: container = &gtk4::Box {
                    set_orientation: Orientation::Vertical,
                    set_halign: Align::Center,
                    append: image = &Image {
                        set_margin_bottom: 4,
                        set_margin_top: 4,
                        set_pixel_size: 32,
                    },
                    append: name = &Label {
                        set_halign: Align::Center,
                        set_hexpand: true,
                        set_ellipsize: EllipsizeMode::End,
                        set_width_request: 96,      
                    }
                }
            }
        };
        toggle_button.set_group(Some(toggle_for_group));

        toggle_button.connect_toggled(glib::clone!(@weak self_ => move|toggle_button| {
            if toggle_button.is_active() {
                self_.emit_by_name::<()>("group-selected", &[&self_.imp().position.get()]);
            }
        }));

        self_.append(&toggle_button);
        imp.name.replace(name);
        imp.image.replace(image);
        imp.button.replace(toggle_button);

        self_
    }

    pub fn set_position(&self, i: u32) {
        self.imp().position.replace(i);
        if i == 0 {
            self.imp().button.borrow().set_active(true);
        }
    }

    pub fn toggle(&self) {
        self.imp().button.borrow().set_active(true);
    }

    pub fn set_group_info(&self, app_group: AppGroup) {
        // if data type set name and icon to values in data
        let imp = imp::GroupItem::from_instance(self);
        match app_group.property::<BoxedAppGroupType>("inner") {
            BoxedAppGroupType::Group(data) => {
                imp.name.borrow().set_text(&data.name);
                imp.image.borrow().set_from_icon_name(Some(&data.icon));
            }
            BoxedAppGroupType::NewGroup(popover_active) => {
                // else must be add group
                imp.name.borrow().set_text(&fl!("new-group"));
                imp.image.borrow().set_from_icon_name(Some("folder-new-symbolic"));

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
                    Label::new(Some(&fl!("name")));
                    ..set_justify(gtk4::Justification::Left);
                    ..set_xalign(0.0);
                };
                popover_menu.append(&label);
                popover_menu.append(&dialog_entry);
                let btn_container = cascade! {
                    gtk4::Box::new(Orientation::Horizontal, 8);
                    ..set_halign(Align::Center);
                };
                let ok_btn = cascade! {
                    Button::with_label(&fl!("ok"));
                    ..add_css_class("suggested-action");
                    ..add_css_class("border-radius-medium");
                };
                let cancel_btn = cascade! {
                    Button::with_label(&fl!("cancel"));
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

                ok_btn.set_sensitive(false);
                dialog_entry.connect_text_notify(glib::clone!(@weak ok_btn => move |entry| {
                    if entry.text().is_empty() {
                        ok_btn.set_sensitive(false);
                    } else {
                        ok_btn.set_sensitive(true);
                    }
                }));
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

    pub fn popup(&self) {
        let imp = imp::GroupItem::from_instance(self);
        if let Some(popover) = imp.popover.borrow().as_ref() {
            popover.popup();
        }
    }

}
