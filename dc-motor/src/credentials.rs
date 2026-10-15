//! Build-time credentials.
//!
//! `build.rs` finds a `credentials.toml`, parses it **at build time**, and
//! writes `$OUT_DIR/credentials.rs`. This module pulls that file in with the
//! `include!` macro, so the values are compiled straight into the firmware —
//! nothing is read from disk at runtime.
//!
//! Search order (first hit wins):
//!   1. `$PICO_CREDENTIALS` (path to a TOML file)
//!   2. `<repo>/secrets/credentials.toml`
//!   3. `<project>/secrets/credentials.toml`
//!   4. `~/.config/picopi/credentials.toml`
//!   5. `secrets/credentials.toml.example` (placeholders, with a build warning)

// Some generated constants may be unused while a project is still growing.
#![allow(dead_code)]

// The generated file defines: WIFI_SSID, WIFI_PASSWORD, WIFI_COUNTRY, SOURCE.
include!(concat!(env!("OUT_DIR"), "/credentials.rs"));

/// Typed, ergonomic access to the baked-in credentials.
///
/// ```ignore
/// let ssid = credentials!(wifi_ssid);
/// ```
#[macro_export]
macro_rules! credentials {
    (wifi_ssid) => {
        $crate::credentials::WIFI_SSID
    };
    (wifi_password) => {
        $crate::credentials::WIFI_PASSWORD
    };
    (wifi_country) => {
        $crate::credentials::WIFI_COUNTRY
    };
    (source) => {
        $crate::credentials::SOURCE
    };
}
