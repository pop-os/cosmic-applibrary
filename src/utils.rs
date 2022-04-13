// SPDX-License-Identifier: MPL-2.0-only
use std::path::PathBuf;

use gtk4::glib;
use gtk4::IconPaintable;
use gtk4::ScrolledWindow;

pub fn data_path() -> PathBuf {
    let mut path = glib::user_data_dir();
    path.push("com.cosmic.app_library");
    std::fs::create_dir_all(&path).expect("Could not create directory.");
    path.push("data.json");
    path
}

pub fn set_group_scroll_policy(scroll_window: &ScrolledWindow, group_cnt: u32) {
    if scroll_window.policy().1 == gtk4::PolicyType::Never && group_cnt > 16 {
        scroll_window.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    } else if scroll_window.policy().1 == gtk4::PolicyType::Automatic && group_cnt <= 16 {
        scroll_window.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Never);
    }
}

pub fn in_flatpak() -> bool {
    std::env::var("FLATPAK_ID").is_ok()
}

pub fn xdg_data_dirs() -> Vec<PathBuf> {
    if in_flatpak() {
        std::str::from_utf8(
            &std::process::Command::new("flatpak-spawn")
                .args(["--host", "printenv", "XDG_DATA_DIRS"])
                .output()
                .unwrap()
                .stdout[..],
        )
        .unwrap_or_default()
        .trim()
        .split(":")
        .map(|p| PathBuf::from(p))
        .collect()
    } else {
        let xdg_base = xdg::BaseDirectories::new().expect("could not access XDG Base directory");
        let mut data_dirs = xdg_base.get_data_dirs();
        data_dirs
    }
}

#[derive(Clone, Debug)]
pub struct DesktopEntryData {
    pub name: String,
    pub appid: String,
    pub icon: String,
}

#[derive(Clone, Debug, Default, glib::Boxed)]
#[boxed_type(name = "BoxedDesktopEntryData")]
pub struct BoxedDesktopEntryData(pub Option<DesktopEntryData>);
