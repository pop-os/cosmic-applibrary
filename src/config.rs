pub const APP_ID: &str = "com.System76.CosmicAppLibrary";
#[cfg(feature = "dev")]
pub const RESOURCES_FILE: &str = concat!("target/compiled.gresource");
#[cfg(not(feature = "dev"))]
pub const RESOURCES_FILE: &str = concat!("/usr/share/com.System76.CosmicAppLibrary/compiled.gresource");
pub const VERSION: &str = "0.0.1";
