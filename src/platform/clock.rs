//! Platform time source.
//!
//! `std::time::{Instant, SystemTime}` COMPILE on wasm32-unknown-unknown but
//! PANIC at runtime (`std::sys::time::unsupported`: "time not implemented on
//! this platform"). The gpui web stack already routes its clocks through the
//! [`web-time`] crate (browser `performance.now()` / `Date`), and app code on
//! the wasm boot/render path must do the same.
//!
//! Import [`Instant`] / [`SystemTime`] / [`UNIX_EPOCH`] from this module
//! instead of `std::time`: native builds get the std types unchanged, wasm
//! builds get the web implementations. `Duration` is platform-independent and
//! safe from either source (it is re-exported here for one-stop imports).
//!
//! [`web-time`]: https://crates.io/crates/web-time

#[cfg(not(target_family = "wasm"))]
pub use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(target_family = "wasm")]
pub use web_time::{Duration, Instant, SystemTime, UNIX_EPOCH};
