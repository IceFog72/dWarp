//! Functionality relating to services that the application provides
//! to the host system.
//!
//! For example, on macOS, this module sets up integrations with
//! Finder such that the user can open a new Warp tab or window
//! in a given directory.

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub mod linux;
#[cfg(target_os = "macos")]
mod mac;
#[cfg(windows)]
pub mod windows;

use warpui::AppContext;

pub fn init(_ctx: &mut AppContext) {
    log::info!("Initializing app services");

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    linux::init(_ctx);
    #[cfg(target_os = "macos")]
    mac::init();
    #[cfg(windows)]
    windows::init(_ctx);
}

pub fn teardown(_ctx: &mut AppContext) {
    log::info!("Tearing down app services...");

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    linux::teardown(_ctx);
}

/// Keep IPC endpoints isolated alongside development data profiles.
#[cfg(any(target_os = "linux", target_os = "freebsd", windows))]
fn instance_profile_suffix() -> String {
    use sha2::{Digest, Sha256};
    use warp_core::channel::ChannelState;

    ChannelState::data_profile()
        .map(|profile| format!(".profile_{:x}", Sha256::digest(profile.as_bytes())))
        .unwrap_or_default()
}
