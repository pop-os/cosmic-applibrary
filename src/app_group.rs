use std::{sync::Arc, vec};

use cosmic::{
    cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry},
    desktop::DesktopEntryData,
};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use crate::{config::APP_ID, fl};

static HOME: LazyLock<[AppGroup; 1]> = LazyLock::new(|| {
    [AppGroup {
        name: "cosmic-library-home".to_string(),
        icon: "user-home-symbolic".to_string(),
        filter: FilterType::None,
    }]
});

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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

impl Ord for FilterType {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (FilterType::AppIds(_), FilterType::AppIds(_)) => std::cmp::Ordering::Equal,
            (FilterType::None, FilterType::None) => std::cmp::Ordering::Equal,
            (FilterType::Categories { .. }, FilterType::Categories { .. }) => {
                std::cmp::Ordering::Equal
            }
            (FilterType::Categories { .. } | FilterType::None, FilterType::AppIds(_)) => {
                std::cmp::Ordering::Less
            }
            (FilterType::AppIds(_), FilterType::Categories { .. } | FilterType::None) => {
                std::cmp::Ordering::Greater
            }
            (FilterType::Categories { .. }, FilterType::None) => std::cmp::Ordering::Greater,
            (FilterType::None, FilterType::Categories { .. }) => std::cmp::Ordering::Less,
        }
    }
}

impl PartialOrd for FilterType {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// Object holding the state
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AppGroup {
    pub name: String,
    pub icon: String,
    pub filter: FilterType,
    // pub popup: bool,
}

impl PartialOrd for AppGroup {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AppGroup {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (&self.filter, &other.filter) {
            (FilterType::AppIds(_), FilterType::AppIds(_)) => {
                self.name.to_lowercase().cmp(&other.name.to_lowercase())
            }
            (FilterType::Categories { categories, .. }, FilterType::AppIds(_)) => {
                if let Some(cat_name) = categories.first() {
                    cat_name.to_lowercase().cmp(&other.name.to_lowercase())
                } else {
                    self.name.to_lowercase().cmp(&other.name.to_lowercase())
                }
            }
            (FilterType::AppIds(_), FilterType::Categories { categories, .. }) => {
                if let Some(other_name) = categories.first() {
                    self.name.to_lowercase().cmp(&other_name.to_lowercase())
                } else {
                    self.name.to_lowercase().cmp(&other.name.to_lowercase())
                }
            }
            (a, b) => a.cmp(b),
        }
    }
}

impl AppGroup {
    pub fn filtered(
        &self,
        input_value: &str,
        exceptions: &[Self],
        all_entries: &[Arc<DesktopEntryData>],
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
                            .iter()
                            .any(|acat| acat.to_lowercase() == input_value.to_lowercase())
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
                exclude,
                ..
            } => {
                categories.iter().any(|cat| {
                    entry
                        .categories
                        .iter()
                        .any(|acat| acat.to_lowercase() == cat.to_lowercase())
                }) && exclude.iter().all(|id| id != &entry.id)
                    || include.iter().any(|id| id == &entry.id)
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
    pub(crate) groups: Vec<AppGroup>,
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
        self.groups.sort();
    }

    pub fn remove(&mut self, i: usize) {
        if i == 0 {
            return;
        }
        if i - 1 < self.groups.len() {
            self.groups.remove(i - 1);
        }
    }

    pub fn set_name(&mut self, i: usize, name: String) {
        if i == 0 {
            return;
        }
        if i - 1 < self.groups.len() {
            self.groups[i - 1].name = name;
        }
    }

    pub fn remove_entry(&mut self, i: usize, id: &str) {
        if i == 0 {
            return;
        }
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
        if i > 0 && i - 1 < self.groups.len() {
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
        } else {
            // add to filter of all groups, forcing it to the Home group
            for group in &mut self.groups {
                match &mut group.filter {
                    FilterType::AppIds(ids) => {
                        ids.retain(|conf_id| conf_id != id);
                    }
                    FilterType::Categories {
                        categories: _,
                        exclude,
                        include,
                    } => {
                        include.retain(|conf_id| conf_id != id);
                        if exclude.iter().all(|conf_id| conf_id != id) {
                            exclude.push(id.to_string());
                        }
                    }
                    FilterType::None => {}
                }
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
                        include: vec![
                            "org.gnome.Totem".to_string(),
                            "org.gnome.eog".to_string(),
                            "simple-scan".to_string(),
                            "thunderbird".to_string(),
                        ],
                        exclude: Vec::new(),
                    },
                },
                AppGroup {
                    name: "cosmic-system".to_string(),
                    icon: "folder-symbolic".to_string(),
                    filter: FilterType::Categories {
                        categories: vec!["System".to_string()],
                        include: vec![
                            "gnome-language-selector".to_string(),
                            "im-config".to_string(),
                            "org.freedesktop.IBus.Setup".to_string(),
                            "system76-driver".to_string(),
                        ],
                        exclude: vec![
                            "com.system76.CosmicStore".to_string(),
                            "com.system76.CosmicTerm".to_string(),
                        ],
                    },
                },
                AppGroup {
                    name: "cosmic-utilities".to_string(),
                    icon: "folder-symbolic".to_string(),
                    filter: FilterType::Categories {
                        categories: vec!["Utility".to_string()],
                        include: vec!["nm-connection-editor".to_string()],
                        exclude: vec![
                            "com.system76.CosmicEdit".to_string(),
                            "com.system76.CosmicFiles".to_string(),
                        ],
                    },
                },
            ],
        }
    }
}
