use gtk4::{glib::{self, subclass::Signal}, subclass::prelude::*, ToggleButton, Popover, prelude::StaticType};
use std::{cell::{RefCell, Cell}, rc::Rc};
use once_cell::sync::Lazy;

// Object holding the state
#[derive(Default)]
pub struct GroupItem {
    pub button: Rc<RefCell<ToggleButton>>,
    pub(super) name: Rc<RefCell<gtk4::Label>>,
    pub(super) image: Rc<RefCell<gtk4::Image>>,
    pub(super) position: Cell<u32>,
    pub(super) popover: Rc<RefCell<Option<Popover>>>,
}

// The central trait for subclassing a GObject
#[glib::object_subclass]
impl ObjectSubclass for GroupItem {
    const NAME: &'static str = "GroupItem";
    type Type = super::GroupItem;
    type ParentType = gtk4::Box;
}

// Trait shared by all GObjects
impl ObjectImpl for GroupItem {
    fn signals() -> &'static [Signal] {
        static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
            vec![
                Signal::builder("new-group")
                    .param_types(Some(String::static_type()))
                    .build(),
                Signal::builder("group-selected")
                .param_types(Some(u32::static_type()))
                .build(),
                Signal::builder("popover-closed").build(),
            ]
        });
        SIGNALS.as_ref()
    }
}
// Trait shared by all widgets
impl WidgetImpl for GroupItem {}

impl BoxImpl for GroupItem {}
