use std::sync::Arc;
use std::vec;

use cosmic::cosmic_config::cosmic_config_derive::CosmicConfigEntry;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::desktop::DesktopEntryData;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::config::APP_ID;
use crate::fl;

static HOME: Lazy<[AppGroup; 1]> = Lazy::new(|| {
    [AppGroup {
        name: "cosmic-library-home".to_string(),
        icon: "user-home-symbolic".to_string(),
        filter: FilterType::None,
    }]
});

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppGroup {
    pub name: String,
    pub icon: String,
    pub filter: FilterType,
    // pub popup: bool,
}

impl AppGroup {
    pub fn filtered(
        &self,
        input_value: &str,
        exceptions: &Vec<Self>,
        all_entries: &Vec<Arc<DesktopEntryData>>,
    ) -> Vec<Arc<DesktopEntryData>> {
        all_entries
            .iter()
            .filter(|de| {
                let mut keep_de = self.matches(de);
                keep_de &= if input_value.is_empty() {
                    !exceptions.iter().any(|x| x.matches(de))
                } else {
                    de.name.to_lowercase().contains(&input_value.to_lowercase())
                        || de
                            .categories
                            .to_lowercase()
                            .contains(&input_value.to_lowercase())
                };
                keep_de
            })
            .cloned()
            .collect()
    }

    fn matches(&self, entry: &DesktopEntryData) -> bool {
        match &self.filter {
            FilterType::AppIds(names) => names.iter().any(|id| id == &entry.id),
            FilterType::Categories {
                categories,
                include,
                ..
            } => {
                categories.iter().any(|cat| {
                    entry
                        .categories
                        .to_lowercase()
                        .contains(&cat.to_lowercase())
                }) || include.iter().any(|id| id == &entry.id)
            }
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

impl AppLibraryConfig {
    pub fn version() -> u64 {
        1
    }

    pub fn helper() -> Option<cosmic_config::Config> {
        cosmic_config::Config::new(APP_ID, Self::version()).ok()
    }

    pub fn add(&mut self, name: String) {
        self.groups.push(AppGroup {
            name,
            icon: "folder-symbolic".to_string(),
            filter: FilterType::AppIds(Vec::new()),
        });
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

    pub fn remove_entry(&mut self, i: usize, id: &str) {
        if let Some(group) = self.groups.get_mut(i - 1) {
            match &mut group.filter {
                FilterType::AppIds(ids) => ids.retain(|conf_id| conf_id != id),
                FilterType::Categories {
                    exclude, include, ..
                } => {
                    include.retain(|conf_id| conf_id != id);
                    exclude.retain(|conf_id| conf_id != id);
                    exclude.push(id.to_string());
                }
                FilterType::None => {}
            }
        }
        if i - 1 < self.groups.len() {
            if let FilterType::AppIds(ids) = &mut self.groups[i - 1].filter {
                ids.retain(|x| x != id);
            }
        }
    }

    pub fn add_entry(&mut self, i: usize, id: &str) {
        if i - 1 < self.groups.len() {
            if let FilterType::AppIds(ids) = &mut self.groups[i - 1].filter {
                if ids.iter().all(|s| s != id) {
                    ids.push(id.to_string());
                }
            } else if let FilterType::Categories {
                exclude, include, ..
            } = &mut self.groups[i - 1].filter
            {
                include.retain(|conf_id| conf_id != id);
                exclude.retain(|conf_id| conf_id != id);
                include.push(id.to_string());
            }
        }
    }

    pub fn groups(&self) -> Vec<&AppGroup> {
        HOME.iter().chain(&self.groups).collect()
    }

    pub fn filtered(
        &self,
        i: usize,
        input_value: &str,
        entries: &Vec<Arc<DesktopEntryData>>,
    ) -> Vec<Arc<DesktopEntryData>> {
        if i == 0 {
            HOME[0].filtered(input_value, &self.groups, entries)
        } else {
            self._filtered(i - 1, input_value, entries)
        }
    }

    pub fn _filtered(
        &self,
        i: usize,
        input_value: &str,
        entries: &Vec<Arc<DesktopEntryData>>,
    ) -> Vec<Arc<DesktopEntryData>> {
        self.groups
            .get(i)
            .map(|g| g.filtered(input_value, &Vec::new(), entries))
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
