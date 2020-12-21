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

/// KMsg Implementation (reads from the /dev/kmsg file)
pub mod kmsgfile;

use std::iter::Iterator;

#[derive(Clone, Copy, Debug)]
pub enum Backend {
    Default,
    KLogCtl,
    DevKMsg,
}

type EntriesIterator = Box<dyn Iterator<Item = Result<entry::Entry, error::RMesgError>>>;

pub fn log_entries(b: Backend, clear: bool) -> Result<Vec<entry::Entry>, error::RMesgError> {
    match b {
        Backend::Default => match kmsgfile::kmsg(None) {
            Ok(e) => Ok(e),
            Err(error::RMesgError::DevKMsgFileOpenError(s)) => {
                eprintln!(
                    "Falling back from device file to klogctl syscall due to error: {}",
                    s
                );
                klogctl::klog(clear)
            }
            Err(e) => Err(e),
        },
        Backend::KLogCtl => klogctl::klog(clear),
        Backend::DevKMsg => kmsgfile::kmsg(None),
    }
}

pub fn logs_raw(b: Backend, clear: bool) -> Result<String, error::RMesgError> {
    match b {
        Backend::Default => match kmsgfile::kmsg_raw(None) {
            Ok(e) => Ok(e),
            Err(error::RMesgError::DevKMsgFileOpenError(s)) => {
                eprintln!(
                    "Falling back from device file to klogctl syscall due to error: {}",
                    s
                );
                klogctl::klog_raw(clear)
            }
            Err(e) => Err(e),
        },
        Backend::KLogCtl => klogctl::klog_raw(clear),
        Backend::DevKMsg => kmsgfile::kmsg_raw(None),
    }
}

pub fn logs_iter(b: Backend, clear: bool, raw: bool) -> Result<EntriesIterator, error::RMesgError> {
    match b {
        Backend::Default => match kmsgfile::KMsgEntries::with_options(None, raw) {
            Ok(e) => Ok(Box::new(e)),
            Err(error::RMesgError::DevKMsgFileOpenError(s)) => {
                eprintln!(
                    "Falling back from device file to klogctl syscall due to error: {}",
                    s
                );
                klog_entries_only_if_timestamp_enabled(clear)
            }
            Err(e) => Err(e),
        },
        Backend::KLogCtl => klog_entries_only_if_timestamp_enabled(clear),
        Backend::DevKMsg => Ok(Box::new(kmsgfile::KMsgEntries::with_options(None, raw)?)),
    }
}

fn klog_entries_only_if_timestamp_enabled(clear: bool) -> Result<EntriesIterator, error::RMesgError> {
    let log_timestamps_enabled = klogctl::klog_timestamps_enabled()?;

    // ensure timestamps in logs
    if !log_timestamps_enabled {
        eprintln!("WARNING: Timestamps are disabled but tailing/following logs (as you've requested) requires them.");
        eprintln!("Aboring program.");
        eprintln!("You can enable timestamps by running the following: ");
        eprintln!("  echo Y > /sys/module/printk/parameters/time");
        return Err(error::RMesgError::KLogTimestampsDisabled);
    }

    Ok(Box::new(klogctl::KLogEntries::with_options(clear, klogctl::SUGGESTED_POLL_INTERVAL)?))
}
