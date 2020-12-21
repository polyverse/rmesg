#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate enum_display_derive;
#[macro_use]
extern crate num_derive;

mod common;

pub mod entry;
pub mod error;

/// KLog Implementation (makes klogctl aka syslog system call through libc)
pub mod klogctl;
