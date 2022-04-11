// SPDX-License-Identifier: MPL-2.0-only

use gtk4::{
    gdk::glib::ParamSpecBoxed,
    glib::{self, ParamFlags, ParamSpec, Value},
    prelude::*,
    subclass::prelude::*,
};
use once_cell::sync::Lazy;
use std::{cell::RefCell, rc::Rc};

use super::BoxedAppGroupType;

// Object holding the state
#[derive(Default)]
pub struct AppGroup {
    pub inner: Rc<RefCell<BoxedAppGroupType>>,
}

// The central trait for subclassing a GObject
#[glib::object_subclass]
impl ObjectSubclass for AppGroup {
    const NAME: &'static str = "AppGroup";
    type Type = super::AppGroup;
    type ParentType = glib::Object;
}

// Trait shared by all GObjects
impl ObjectImpl for AppGroup {
    fn properties() -> &'static [ParamSpec] {
        static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
            vec![ParamSpecBoxed::new(
                // Name
                "inner",
                // Nickname
                "inner",
                // Short description
                "inner",
                BoxedAppGroupType::static_type(),
                // The property can be read and written to
                ParamFlags::READWRITE,
            )]
        });
        PROPERTIES.as_ref()
    }

    fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
        match pspec.name() {
            "inner" => {
                let inner = value.get().expect("The value needs to be of type `u32`.");
                self.inner.replace(inner);
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
        match pspec.name() {
            "inner" => self.inner.borrow().to_value(),
            _ => unimplemented!(),
        }
    }
}
