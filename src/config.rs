pub const APP_ID: &str = "com.system76.CosmicAppLibrary";
pub const VERSION: &str = "0.1.0";

pub fn profile() -> &'static str {
    std::env!("OUT_DIR")
        .split(std::path::MAIN_SEPARATOR)
        .nth_back(3)
        .unwrap_or("unknown")
}
