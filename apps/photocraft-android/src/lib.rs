//! Native Android launcher for the shared PhotoCraft Rust editor.
#![deny(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
mod surface;

// SAFETY: android-activity requires this exact, unique Rust-ABI symbol. It is
// invoked with AndroidApp by that crate; the body contains only safe Rust.
// This attribute-only exception was explicitly approved by the user.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_tag("PhotoCraft").with_max_level(log::LevelFilter::Info));
    if let Err(error) = android::run(app) {
        log::error!("PhotoCraft could not start: {error}");
    }
}

#[cfg(target_os = "android")]
#[path = "../../photocraft/src/control_server.rs"]
mod control_server;
#[path = "../../photocraft/src/crash_guard.rs"]
pub mod crash_guard;
#[cfg(any(target_os = "android", test))]
mod fullscreen;
pub mod picker;
pub mod recovery;
pub mod services;
pub mod storage;
pub mod viewport;
