use std::borrow::Cow;
use std::path::PathBuf;
use std::vec;

use cosmic::cosmic_config::cosmic_config_derive::CosmicConfigEntry;
use cosmic::cosmic_config::{self, Config, ConfigGet, ConfigSet, CosmicConfigEntry};
use freedesktop_desktop_entry::DesktopEntry;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::fl;

static HOME: Lazy<[AppGroup; 1]> = Lazy::new(|| {
    [AppGroup {
        name: "cosmic-library-home".to_string(),
        icon: "user-home-symbolic".to_string(),
        filter: FilterType::None,
    }]
});

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FilterType {
    /// A list of application IDs to include in the group.
    AppIds(Vec<String>),
    Categories {
        categories: Vec<String>,
        /// The ID of applications which may not match the categories, but should be included anyway.
        exclude: Vec<String>,
        /// The ID of applications which should be excluded from the results.
        include: Vec<String>,
    },
    /// No filter is applied.
    /// This is intended for use with Home.
    None,
}

impl Default for FilterType {
    fn default() -> Self {
        FilterType::AppIds(Vec::new())
    }
}

// Object holding the state
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct AppGroup {
    pub name: String,
    pub icon: String,
    pub filter: FilterType,
    // pub popup: bool,
}

impl AppGroup {
    pub fn filtered(
        &self,
        locale: Option<&str>,
        input_value: &str,
        exceptions: &Vec<Self>,
    ) -> Vec<MyDesktopEntryData> {
        freedesktop_desktop_entry::Iter::new(freedesktop_desktop_entry::default_paths())
            .filter_map(|path| {
                std::fs::read_to_string(&path).ok().and_then(|input| {
                    DesktopEntry::decode(&path, &input).ok().and_then(|de| {
                        let name = de
                            .name(locale.as_ref().map(|x| &**x))
                            .unwrap_or(Cow::Borrowed(de.appid))
                            .to_string();
                        let Some(exec) = de.exec() else {
                            return None;
                        };
                        let mut keep_de = !de.no_display()
                            && self.matches(&de)
                            && !exceptions.iter().any(|x| x.matches(&de));
                        if keep_de && input_value.len() > 0 {
                            keep_de = name.to_lowercase().contains(&input_value.to_lowercase())
                                || de
                                    .categories()
                                    .map(|cats| {
                                        cats.to_lowercase().contains(&input_value.to_lowercase())
                                    })
                                    .unwrap_or_default()
                        }
                        if keep_de {
                            freedesktop_icons::lookup(de.icon().unwrap_or(de.appid))
                                .with_size(72)
                                .with_cache()
                                .find()
                                .map(|icon| MyDesktopEntryData {
                                    exec: exec.to_string(),
                                    name,
                                    icon,
                                })
                        } else {
                            None
                        }
                    })
                })
            })
            .collect()
    }

    fn matches(&self, entry: &DesktopEntry) -> bool {
        match &self.filter {
            FilterType::AppIds(names) => names.iter().any(|id| id == entry.appid),
            FilterType::Categories { categories, .. } => categories.into_iter().any(|cat| {
                entry
                    .categories()
                    .map(|cats| cats.to_lowercase().contains(&cat.to_lowercase()))
                    .unwrap_or_default()
            }),
            FilterType::None => true,
        }
    }

    pub fn name(&self) -> String {
        if &self.name == "cosmic-library-home" {
            fl!("cosmic-library-home")
        } else if &self.name == "cosmic-office" {
            fl!("cosmic-office")
        } else if &self.name == "cosmic-system" {
            fl!("cosmic-system")
        } else if &self.name == "cosmic-utilities" {
            fl!("cosmic-utilities")
        } else {
            self.name.clone()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CosmicConfigEntry)]
pub struct AppLibraryConfig {
    groups: Vec<AppGroup>,
}

#[derive(Debug, Clone)]
pub struct MyDesktopEntryData {
    pub exec: String,
    pub name: String,
    pub icon: PathBuf,
}

impl AppLibraryConfig {
    pub fn version() -> u64 {
        1
    }

    pub fn remove(&mut self, i: usize) {
        if i - 1 < self.groups.len() {
            self.groups.remove(i - 1);
        }
    }

    pub fn set_name(&mut self, i: usize, name: String) {
        if i - 1 < self.groups.len() {
            self.groups[i - 1].name = name;
        }
    }

    pub fn groups(&self) -> Vec<&AppGroup> {
        HOME.iter().chain(&self.groups).collect()
    }

    pub fn filtered(
        &self,
        i: usize,
        locale: Option<&str>,
        input_value: &str,
    ) -> Vec<MyDesktopEntryData> {
        if i == 0 {
            HOME[0].filtered(locale, input_value, &self.groups)
        } else {
            self._filtered(i - 1, locale, input_value)
        }
    }

    pub fn _filtered(
        &self,
        i: usize,
        locale: Option<&str>,
        input_value: &str,
    ) -> Vec<MyDesktopEntryData> {
        self.groups
            .get(i)
            .map(|g| g.filtered(locale, input_value, &Vec::new()))
            .unwrap_or_default()
    }
}

impl Default for AppLibraryConfig {
    fn default() -> Self {
        AppLibraryConfig {
            groups: vec![
                AppGroup {
                    name: "cosmic-office".to_string(),
                    icon: "folder-symbolic".to_string(),
                    filter: FilterType::Categories {
                        categories: vec!["Office".to_string()],
                        include: Vec::new(),
                        exclude: Vec::new(),
                    },
                },
                AppGroup {
                    name: "cosmic-system".to_string(),
                    icon: "folder-symbolic".to_string(),
                    filter: FilterType::Categories {
                        categories: vec!["System".to_string()],
                        include: Vec::new(),
                        exclude: Vec::new(),
                    },
                },
                AppGroup {
                    name: "cosmic-utilities".to_string(),
                    icon: "folder-symbolic".to_string(),
                    filter: FilterType::Categories {
                        categories: vec!["Utility".to_string()],
                        include: Vec::new(),
                        exclude: Vec::new(),
                    },
                },
            ],
        }
    }
}
