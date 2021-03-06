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
pub struct GridItem {
    pub(super) name: Rc<RefCell<gtk4::Label>>,
    pub(super) image: Rc<RefCell<gtk4::Image>>,
    pub(super) index: Cell<u32>,
    pub(super) popover: Rc<RefCell<Option<Popover>>>,
    pub(super) icon_theme: OnceCell<IconTheme>,
}

#[glib::object_subclass]
impl ObjectSubclass for GridItem {
    const NAME: &'static str = "GridItem";
    type Type = super::GridItem;
    type ParentType = gtk4::Box;
}

impl ObjectImpl for GridItem {
    fn signals() -> &'static [Signal] {
        static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
            vec![
                Signal::builder(
                    // Signal name
                    "new-group",
                    // Types of the values which will be sent to the signal handler
                    &[String::static_type().into()],
                    // Type of the value the signal handler sends back
                    <()>::static_type().into(),
                )
                .build(),
                Signal::builder(
                    // Signal name
                    "popover-closed",
                    // Types of the values which will be sent to the signal handler
                    &[],
                    // Type of the value the signal handler sends back
                    <()>::static_type().into(),
                )
                .build(),
            ]
        });
        SIGNALS.as_ref()
    }
}

impl WidgetImpl for GridItem {}

impl BoxImpl for GridItem {}
