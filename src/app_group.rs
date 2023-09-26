use std::borrow::Cow;
use std::path::PathBuf;
use std::vec;

use cosmic::cosmic_config::cosmic_config_derive::CosmicConfigEntry;
use cosmic::cosmic_config::{self, Config, ConfigGet, ConfigSet, CosmicConfigEntry};
use freedesktop_desktop_entry::DesktopEntry;
use itertools::Itertools;
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
    pub fn filtered(&self, locale: Option<&str>, input_value: &str) -> Vec<DesktopEntryData> {
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
                        let mut keep_de = !de.no_display() && self.matches(&de);
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
                            let icon = freedesktop_icons::lookup(de.icon().unwrap_or(de.appid))
                                .with_size(72)
                                .with_cache()
                                .find()
                                .or_else(|| {
                                    freedesktop_icons::lookup("application-default")
                                        .with_size(72)
                                        .with_cache()
                                        .find()
                                })
                                .unwrap_or_default();

                            Some(DesktopEntryData {
                                id: de.appid.to_string(),
                                exec: exec.to_string(),
                                name,
                                icon,
                                path: path.clone(),
                                desktop_actions: de
                                    .actions()
                                    .map(|actions| {
                                        actions
                                            .split(";")
                                            .filter_map(|action| {
                                                let name = de
                                                    .action_entry_localized(action, "Name", locale);
                                                let exec = de.action_entry(action, "Exec");
                                                if let (Some(name), Some(exec)) = (name, exec) {
                                                    Some(DesktopAction {
                                                        name: name.to_string(),
                                                        exec: exec.to_string(),
                                                    })
                                                } else {
                                                    None
                                                }
                                            })
                                            .collect_vec()
                                    })
                                    .unwrap_or_default(),
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
            FilterType::Categories {
                categories,
                include,
                ..
            } => {
                categories.into_iter().any(|cat| {
                    entry
                        .categories()
                        .map(|cats| cats.to_lowercase().contains(&cat.to_lowercase()))
                        .unwrap_or_default()
                }) || include.iter().any(|id| id == entry.appid)
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

#[derive(Debug, Clone, PartialEq)]
pub struct DesktopAction {
    pub name: String,
    pub exec: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DesktopEntryData {
    pub id: String,
    pub exec: String,
    pub name: String,
    pub icon: PathBuf,
    pub path: PathBuf,
    pub desktop_actions: Vec<DesktopAction>,
}

impl TryFrom<PathBuf> for DesktopEntryData {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let input = std::fs::read_to_string(&path)?;
        let de = DesktopEntry::decode(&path, &input)?;
        let name = de.name(None).unwrap_or(Cow::Borrowed(de.appid)).to_string();
        let Some(exec) = de.exec() else {
            anyhow::bail!("No exec found in desktop entry")
        };
        let Some(icon) = de.icon() else {
            anyhow::bail!("No icon found in desktop entry")
        };
        Ok(DesktopEntryData {
            id: de.appid.to_string(),
            exec: exec.to_string(),
            name,
            icon: icon.into(),
            path: path.clone(),
            desktop_actions: de
                .actions()
                .map(|actions| {
                    actions
                        .split(";")
                        .filter_map(|action| {
                            let name = de.action_entry_localized(action, "name", None);
                            let exec = de.action_entry(action, "exec");
                            if let (Some(name), Some(exec)) = (name, exec) {
                                Some(DesktopAction {
                                    name: name.to_string(),
                                    exec: exec.to_string(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect_vec()
                })
                .unwrap_or_default(),
        })
    }
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
        locale: Option<&str>,
        input_value: &str,
    ) -> Vec<DesktopEntryData> {
        if i == 0 {
            HOME[0].filtered(locale, input_value)
        } else {
            self._filtered(i - 1, locale, input_value)
        }
    }

    pub fn _filtered(
        &self,
        i: usize,
        locale: Option<&str>,
        input_value: &str,
    ) -> Vec<DesktopEntryData> {
        self.groups
            .get(i)
            .map(|g| g.filtered(locale, input_value))
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
