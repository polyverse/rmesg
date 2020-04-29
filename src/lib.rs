#[cfg(target_os = "linux")]
#[macro_use]
extern crate enum_display_derive;

pub mod error;

// Export Linux when possible
#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "linux")]
pub use linux::rmesg;

// Export default when none is possible
#[cfg(not(target_os = "linux"))]
pub mod default;

#[cfg(not(target_os = "linux"))]
pub use default::rmesg;
