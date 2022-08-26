// SPDX-License-Identifier: MPL-2.0-only
use glib::subclass::Signal;
use gtk4::subclass::prelude::*;
use gtk4::IconTheme;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{glib, prelude::*, Popover};

#[derive(Debug, Default)]
pub struct AppItem {
    pub(super) name: Rc<RefCell<gtk4::Label>>,
    pub(super) image: Rc<RefCell<gtk4::Image>>,
    pub(super) index: Cell<u32>,
    pub(super) _popover: Rc<RefCell<Option<Popover>>>,
    pub(super) icon_theme: OnceCell<IconTheme>,
}

#[glib::object_subclass]
impl ObjectSubclass for AppItem {
    const NAME: &'static str = "AppItem";
    type Type = super::AppItem;
    type ParentType = gtk4::Box;
}

impl ObjectImpl for AppItem {
    fn signals() -> &'static [Signal] {
        static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
            vec![
                Signal::builder("new-group")
                    .param_types(Some(String::static_type()))
                    .build(),
                Signal::builder("popover-closed").build(),
            ]
        });
        SIGNALS.as_ref()
    }
}

impl WidgetImpl for AppItem {}

impl BoxImpl for AppItem {}
