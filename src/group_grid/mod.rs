// SPDX-License-Identifier: MPL-2.0-only
use cascade::cascade;
use gtk4::{
    gio,
    glib::{self, Object},
    prelude::*,
    subclass::prelude::*,
    GridView, PolicyType, ScrolledWindow, SignalListItemFactory, ToggleButton, ListItem,
};
use std::fs::File;

use crate::{utils::data_path, app_group::FilterType};
use crate::utils::set_group_scroll_policy;
use crate::{
    app_group::{AppGroup, AppGroupData, BoxedAppGroupType},
    desktop_entry_data::DesktopEntryData,
};
use crate::{fl, group_item::GroupItem};

mod imp;

glib::wrapper! {
    pub struct GroupGrid(ObjectSubclass<imp::GroupGrid>)
        @extends gtk4::Widget, gtk4::Box,
    @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl Default for GroupGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl GroupGrid {
    pub fn new() -> Self {
        let self_: Self = glib::Object::new(&[]).expect("Failed to create GroupGrid");
        let imp = imp::GroupGrid::from_instance(&self_);

        let group_window = cascade! {
            ScrolledWindow::new();
            ..set_hscrollbar_policy(PolicyType::Never);
            ..set_vscrollbar_policy(PolicyType::Never);
            ..set_propagate_natural_height(true);
            ..set_min_content_height(150);
            ..set_max_content_height(300);
            ..set_hexpand(true);
            ..add_css_class("primary-container");
        };
        self_.append(&group_window);

        let group_grid_view = cascade! {
            GridView::default();
            ..set_min_columns(8);
            ..set_max_columns(8);
            ..add_css_class("primary-container");
        };
        group_window.set_child(Some(&group_grid_view));

        imp.group_grid_view.set(group_grid_view).unwrap();
        imp.group_scroll_window.set(group_window).unwrap();

        // Setup
        // Setup
        self_.setup_model();
        self_.restore_data();
        self_.setup_callbacks();
        self_.setup_factory();

        self_
    }

    pub fn reset(&self) {
        self.group_model().items_changed(0, 1, 1);
    }

    fn setup_model(&self) {
        let imp = imp::GroupGrid::from_instance(&self);
        let group_model = gio::ListStore::new(AppGroup::static_type());
        imp.group_model
            .set(group_model.clone())
            .expect("Could not set group model");
        vec![
            AppGroup::new(BoxedAppGroupType::Group(AppGroupData {
                id: 0,
                name: fl!("library-home"),
                icon: "user-home".to_string(),
                mutable: false,
                filter: FilterType::None,
            })),
            AppGroup::new(BoxedAppGroupType::Group(AppGroupData {
                id: 0,
                name: fl!("system"),
                icon: "folder".to_string(),
                mutable: false,
                filter: FilterType::Categories(vec!["System".to_string()]),
            })),
            AppGroup::new(BoxedAppGroupType::Group(AppGroupData {
                id: 0,
                name: fl!("utilities"),
                icon: "folder".to_string(),
                mutable: false,
                filter: FilterType::Categories(vec!["Utility".to_string()]),
            })),
            AppGroup::new(BoxedAppGroupType::NewGroup(false)),
        ]
        .iter()
        .for_each(|group| {
            group_model.append(group);
        });
        let group_selection = gtk4::NoSelection::new(Some(&group_model));
        imp.group_grid_view
            .get()
            .unwrap()
            .set_model(Some(&group_selection));
    }

    fn group_model(&self) -> &gio::ListStore {
        // Get state
        let imp = imp::GroupGrid::from_instance(self);
        imp.group_model.get().expect("Could not get model")
    }

    fn setup_callbacks(&self) {
        let imp = imp::GroupGrid::from_instance(self);

        let scroll_window = &imp.group_scroll_window.get().unwrap();
        // dynamically set scroll method
        self.group_model().connect_items_changed(
            glib::clone!(@weak scroll_window => move |scroll_list_model, _i, _rmv_cnt, _add_cnt| {
                set_group_scroll_policy(&scroll_window, scroll_list_model.n_items());
            }),
        );
    }

    pub fn is_popup_active(&self) -> bool {
        let model = self.group_model();
        for i in 0..model.n_items() {
            let item = model.item(i).unwrap().downcast::<AppGroup>().unwrap();
            if item.is_popup_active() {
                return true;
            }
        }
        return false;
    }

    fn setup_factory(&self) {
        let dummy_toggle = ToggleButton::new();
        let imp = imp::GroupGrid::from_instance(&self);
        let group_factory = SignalListItemFactory::new();
        group_factory.connect_setup(glib::clone!(@weak self as self_, @strong dummy_toggle => move |_factory, item| {
            item.set_activatable(false);
            let obj = GroupItem::new(&dummy_toggle);
            obj.set_hexpand(true);
            item.set_child(Some(&obj));
            obj
                .connect_local("new-group", false, glib::clone!(@weak self_ => @default-return None, move |args| {
                    let m = self_.group_model();
                    match args[1].get::<String>() {
                        Ok(name) => {
                            let mut i = 0;
                            while let Some(item_name) = m.item(i).and_then(|i| i.downcast::<AppGroup>().ok()).and_then(|g| match g.property::<BoxedAppGroupType>("inner") {
                                BoxedAppGroupType::Group(g) => Some(g.name),
                                BoxedAppGroupType::NewGroup(_) => None,
                            }) {
                                if &item_name == &name
                                {
                                    // TODO: toast that the name already exists.
                                    return None;
                                }
                                i += 1;
                            }
                            let new_group = AppGroup::new(BoxedAppGroupType::Group(AppGroupData {
                                id: 0,
                                name: name,
                                icon: "folder".to_string(),
                                mutable: false,
                                filter: FilterType::AppNames(Vec::new())
                            })).upcast::<Object>();
                            m.insert(m.n_items() - 1, &new_group);
                            self_.store_data();
                        }
                        _ => unimplemented!(),
                    };
                    None
                }));
            obj
                .connect_local("popover-closed", false, glib::clone!(@weak self_ => @default-return None, move |_| {
                    let m = self_.group_model();
                    let group = m.item(m.n_items() - 1).unwrap().downcast::<AppGroup>().unwrap();
                    group.popdown();
                    self_.reset();
                    None
                }));

            obj.connect_closure("group-selected", false, glib::closure_local!(@weak-allow-none self_,  => move |_: GroupItem, i: u32| {
                    // on activation change the group filter model to use the app names, and category
                    let self_ = match self_ {
                        Some(s) => s,
                        None => return,
                    };
                    let group_model = self_.group_model();
                    // update the application filter
                    if let Some(data) = group_model
                        .item(i)
                        .unwrap()
                        .downcast::<AppGroup>()
                        .unwrap()
                        .group_data()
                    {
                        let filter = data.filter;
        
                        let new_filter: gtk4::CustomFilter = gtk4::CustomFilter::new(move |obj| {
                            let app = obj
                                .downcast_ref::<DesktopEntryData>()
                                .expect("The Object needs to be of type AppInfo");
                            match filter {
                                crate::app_group::FilterType::AppNames(ref names) => names.contains(&String::from(app.name().as_str())),
                                crate::app_group::FilterType::Categories(ref requested_categories) => requested_categories.iter().any(|category| app.categories()
                                .to_string()
                                .to_lowercase()
                                .contains(&category.to_lowercase())),
                                crate::app_group::FilterType::None => true,
                            }
                        });
                        self_.emit_by_name::<()>("group-changed", &[&new_filter]);
                    } else {
                        // don't change filter, instead show dialog for adding new group!
                        let item = group_model.item(i).unwrap().downcast::<AppGroup>().unwrap();
                        item.popup();
                        group_model.items_changed(i, 0, 0);
                    }
            }));
        }));

        // the bind stage is used for "binding" the data to the created widgets on the "setup" stage
        group_factory.connect_bind(move |_factory, grid_item| {
            let group_info = grid_item.item().unwrap().downcast::<AppGroup>().unwrap();

            let child = grid_item.child().unwrap().downcast::<GroupItem>().unwrap();
            child.set_group_info(group_info);
            child.set_position(grid_item.position())
        });
        // Set the factory of the list view
        imp.group_grid_view
            .get()
            .unwrap()
            .set_factory(Some(&group_factory));
    }

    fn restore_data(&self) {
        if let Ok(file) = File::open(data_path()) {
            // Deserialize data from file to vector
            let backup_data: Vec<AppGroupData> =
                serde_json::from_reader(file).unwrap_or_default();

            let app_group_objects: Vec<Object> = backup_data
                .into_iter()
                .map(|data| AppGroup::new(BoxedAppGroupType::Group(data)).upcast::<Object>())
                .collect();
            let scroll_window = &imp::GroupGrid::from_instance(self)
                .group_scroll_window
                .get()
                .unwrap();

            // Insert restored objects into model
            self.group_model().splice(3, 0, &app_group_objects);
            set_group_scroll_policy(&scroll_window, self.group_model().n_items());
        }
    }
    pub fn store_data(&self) {
        let mut backup_data = Vec::new();
        let mut position = 3;
        while let Some(item) = self.group_model().item(position) {
            if position == self.group_model().n_items() - 1 {
                break;
            }
            // Get `AppGroup` from `glib::Object`
            let group_data = item
                .downcast_ref::<AppGroup>()
                .expect("The object needs to be of type `AppGroupData`.")
                .group_data();
            // Add data to vector and increase position
            backup_data.push(group_data);
            position += 1;
        }

        // Save state in file
        let file = File::create(data_path()).expect("Could not create json file.");
        serde_json::to_writer_pretty(file, &backup_data)
            .expect("Could not write data to json file");
    }
}
