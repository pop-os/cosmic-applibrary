use std::{sync::Arc, vec};

use cosmic::{
    cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry},
    desktop::DesktopEntryData,
};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use crate::fl;

static ALL_PROGRAMS: LazyLock<[AppGroup; 1]> = LazyLock::new(|| {
    [AppGroup {
        name: "cosmic-all-programs".to_string(),
        icon: "view-grid-symbolic".to_string(),
        filter: FilterType::None,
    }]
});

static HOME: LazyLock<[AppGroup; 1]> = LazyLock::new(|| {
    [AppGroup {
        name: "cosmic-library-home".to_string(),
        icon: "folder-multiple-symbolic".to_string(),
        filter: FilterType::None,
    }]
});

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum FilterType {
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
        FilterType::None
    }
}

impl Ord for FilterType {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (FilterType::None, FilterType::None) => std::cmp::Ordering::Equal,
            (FilterType::Categories { .. }, FilterType::Categories { .. }) => {
                std::cmp::Ordering::Equal
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
        self.name.to_lowercase().cmp(&other.name.to_lowercase())
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
        if &self.name == "cosmic-all-programs" {
            fl!("cosmic-all-programs")
        } else if &self.name == "cosmic-library-home" {
            fl!("cosmic-library-home")
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

    pub fn groups(&self) -> Vec<&AppGroup> {
        ALL_PROGRAMS.iter().chain(&self.groups).chain(HOME.iter()).collect()
    }

    pub fn filtered(
        &self,
        i: usize,
        input_value: &str,
        entries: &Vec<Arc<DesktopEntryData>>,
    ) -> Vec<Arc<DesktopEntryData>> {
        if i == 0 {
            // All Programs
            ALL_PROGRAMS[0].filtered(input_value, &[], entries) // No exceptions
        } else if i <= self.groups.len() {
            // Dynamic categories
            self._filtered(i - 1, input_value, entries)
        } else {
            // Others (Home)
            HOME[0].filtered(input_value, &self.groups, entries)
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
            groups: vec![],
        }
    }
}
