use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub enum FilterType {
    AppNames(Vec<String>),
    Categories(Vec<String>),
    None,
}

impl Default for FilterType {
    fn default() -> Self {
        FilterType::AppNames(Vec::new())
    }
}

// Object holding the state
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct AppGroup {
    pub name: String,
    pub icon: String,
    pub mutable: bool,
    pub filter: FilterType,
    // pub popup: bool,
}

pub fn save(groups: Vec<AppGroup>) -> anyhow::Result<()> {
    todo!()
}

pub fn load() -> Vec<AppGroup> {
    todo!()
}
