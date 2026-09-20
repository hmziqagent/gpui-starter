//! Platform time source: native builds re-export `std::time`; wasm builds
//! re-export `web-time` (std's clocks compile on wasm but panic at runtime).

#[cfg(not(target_family = "wasm"))]
pub use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(target_family = "wasm")]
pub use web_time::{Duration, Instant, SystemTime, UNIX_EPOCH};
