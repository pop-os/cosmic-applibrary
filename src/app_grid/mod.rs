// SPDX-License-Identifier: MPL-2.0-only

use cascade::cascade;
use freedesktop_desktop_entry::DesktopEntry;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{gdk, gio, glib, GridView, PolicyType, ScrolledWindow, SignalListItemFactory};
use std::{ffi::OsStr, fs, path::Path};
use walkdir::WalkDir;

use crate::utils;
use crate::{desktop_entry_data::DesktopEntryData, grid_item::GridItem};

mod imp;

glib::wrapper! {
    pub struct AppGrid(ObjectSubclass<imp::AppGrid>)
        @extends gtk4::Widget, gtk4::Box,
    @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl Default for AppGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl AppGrid {
    pub fn new() -> Self {
        let self_: Self = glib::Object::new(&[]).expect("Failed to create AppGrid");
        let imp = imp::AppGrid::from_instance(&self_);

        let library_window = cascade! {
            ScrolledWindow::new();
            ..set_hscrollbar_policy(PolicyType::Never);
            ..set_min_content_height(520);
            ..set_hexpand(true);
            ..set_margin_top(12);
            ..add_css_class("primary-container");
        };
        self_.append(&library_window);

        let library_grid = cascade! {
            GridView::default();
            ..set_min_columns(7);
            ..set_max_columns(7);
            ..set_single_click_activate(true);
            ..add_css_class("primary-container");
        };
        library_window.set_child(Some(&library_grid));

        imp.app_grid_view.set(library_grid).unwrap();
        let xdg_base = xdg::BaseDirectories::new().expect("could not access XDG Base directory");

        let icon_theme = gtk4::IconTheme::for_display(&gdk::Display::default().unwrap());
        let mut data_dirs = utils::xdg_data_dirs();

        data_dirs.push(xdg_base.get_data_home());
        if utils::in_flatpak() {
            for mut p in data_dirs {
                if p.starts_with("/usr") {
                    let stripped_path = p.strip_prefix("/").unwrap_or(&p);
                    p = Path::new("/var/run/host").join(stripped_path);
                }
                let mut icons = p.clone();
                icons.push("icons");
                let mut pixmaps = p.clone();
                pixmaps.push("pixmaps");

                icon_theme.add_search_path(icons);
                icon_theme.add_search_path(pixmaps);
            }
        }
        // dbg!(icon_theme.search_path());
        // dbg!(icon_theme.icon_names());
        imp.icon_theme.set(icon_theme).unwrap();

        // Setup
        self_.setup_model();
        self_.setup_callbacks();
        self_.setup_factory();

        self_
    }

    fn setup_model(&self) {
        // Create new model
        let app_model = gio::ListStore::new(DesktopEntryData::static_type());
        // Get state and set model
        let imp = imp::AppGrid::from_instance(self);

        let xdg_base = xdg::BaseDirectories::new().expect("could not access XDG Base directory");

        let mut data_dirs = utils::xdg_data_dirs();
        data_dirs.push(xdg_base.get_data_home());
        if utils::in_flatpak() {
            data_dirs.iter_mut().for_each(|p| {
                if p.starts_with("/usr") {
                    let stripped_path = p.strip_prefix("/").unwrap_or(&p);
                    *p = Path::new("/var/run/host").join(stripped_path);
                }
            });
        }

        data_dirs.iter_mut().for_each(|xdg_data_path| {
            for entry in WalkDir::new(xdg_data_path)
                .max_depth(2)
                .into_iter()
                .filter_map(|e| {
                    if let Ok(e) = e {
                        let p = e.into_path();
                        if p.extension() == Some(OsStr::new("desktop")) {
                            Some(p)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            {
                if let Ok(bytes) = fs::read_to_string(&entry) {
                    if let Ok(de) = DesktopEntry::decode(&entry, &bytes) {
                        let name: String = de.name(None).unwrap_or_default().into();
                        if name.eq("".into()) || de.no_display() {
                            continue;
                        };
                        // dbg!(de.appid);
                        let app_info = DesktopEntryData::new();
                        app_info.set_data(
                            entry
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into(),
                            entry.clone(),
                            name,
                            de.icon().map(|s| String::from(s)),
                            de.categories().unwrap_or_default().into(),
                        );
                        // dbg!((
                        //     &app_info.appid(),
                        //     &app_info.name(),
                        //     &app_info.icon(),
                        //     &app_info.categories(),
                        // ));
                        app_model.append(&app_info);
                    }
                }
            }
        });

        // A sorter used to sort AppInfo in the model by their name
        let sorter = gtk4::CustomSorter::new(move |obj1, obj2| {
            let app_info1 = obj1.downcast_ref::<DesktopEntryData>().unwrap();
            let app_info2 = obj2.downcast_ref::<DesktopEntryData>().unwrap();

            app_info1
                .name()
                .to_lowercase()
                .cmp(&app_info2.name().to_lowercase())
                .into()
        });
        let filter = gtk4::CustomFilter::new(|_obj| true);

        let search_filter_model =
            gtk4::FilterListModel::new(Some(&app_model), Some(filter).as_ref());
        let filter = gtk4::CustomFilter::new(|_obj| true);
        let group_filter_model =
            gtk4::FilterListModel::new(Some(&search_filter_model), Some(filter).as_ref());
        let sorted_model = gtk4::SortListModel::new(Some(&group_filter_model), Some(&sorter));

        let selection_model = gtk4::SingleSelection::builder()
            .model(&sorted_model)
            .autoselect(false)
            .can_unselect(true)
            .selected(gtk4::INVALID_LIST_POSITION)
            .build();

        // Wrap model with selection and pass it to the list view
        imp.app_model
            .set(app_model.clone())
            .expect("Could not set model");
        imp.app_sort_model.set(sorted_model).unwrap();
        imp.search_filter_model.set(search_filter_model).unwrap();
        imp.group_filter_model.set(group_filter_model).unwrap();
        imp.app_grid_view
            .get()
            .unwrap()
            .set_model(Some(&selection_model));
        selection_model.unselect_all();
    }

    fn setup_callbacks(&self) {
        let imp = imp::AppGrid::from_instance(self);
        let app_grid_view = &imp.app_grid_view.get().unwrap();

        app_grid_view.connect_activate(move |list_view, i| {
            // on activation change the group filter model to use the app names, and category
            // println!("selected app {}", i);
            // Launch the application when an item of the list is activated
            let model = list_view.model().unwrap();
            if let Some(item) = model.item(i) {
                let app_info = item.downcast::<DesktopEntryData>().unwrap();
                // TODO include context in launch
                let context = list_view.display().app_launch_context();
                if let Err(err) = app_info.launch() {
                    gtk4::MessageDialog::builder()
                        .text(&format!("Failed to start {}", app_info.name()))
                        .secondary_text(&err.to_string())
                        .message_type(gtk4::MessageType::Error)
                        .modal(true)
                        .build()
                        .show();
                }
            }
        });
    }

    fn setup_factory(&self) {
        let imp = imp::AppGrid::from_instance(&self);
        let app_factory = SignalListItemFactory::new();
        let icon_theme = &imp.icon_theme.get().unwrap();
        app_factory.connect_setup(glib::clone!(@weak icon_theme => move |_factory, item| {
            let grid_item = GridItem::new();
            grid_item.set_icon_theme(icon_theme);
            item.set_child(Some(&grid_item));
        }));

        let imp = imp::AppGrid::from_instance(self);
        // the bind stage is used for "binding" the data to the created widgets on the "setup" stage
        let app_grid_view = &imp.app_grid_view.get().unwrap();
        app_factory.connect_bind(
            glib::clone!(@weak app_grid_view => move |_factory, grid_item| {
                let app_info = grid_item
                    .item()
                    .unwrap()
                    .downcast::<DesktopEntryData>()
                    .unwrap();
                let child = grid_item.child().unwrap().downcast::<GridItem>().unwrap();
                child.set_desktop_entry_data(&app_info);
            }),
        );
        // Set the factory of the list view
        app_grid_view.set_factory(Some(&app_factory));
    }

    pub fn set_app_sorter(&self, sorter: &gtk4::CustomSorter) {
        let imp = imp::AppGrid::from_instance(&self);
        let sort_model = imp.app_sort_model.get().unwrap();
        sort_model.set_sorter(Some(sorter));
    }

    pub fn set_search_filter(&self, filter: &gtk4::CustomFilter) {
        let imp = imp::AppGrid::from_instance(&self);
        let filter_model = imp.search_filter_model.get().unwrap();
        filter_model.set_filter(Some(filter));
    }

    pub fn set_group_filter(&self, filter: &gtk4::CustomFilter) {
        let imp = imp::AppGrid::from_instance(&self);
        let filter_model = imp.group_filter_model.get().unwrap();
        filter_model.set_filter(Some(filter));
    }
}
