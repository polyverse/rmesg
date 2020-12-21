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

#[derive(Clone, Copy, Debug)]
pub enum Backend {
    Default,
    KLogCtl,
    DevKMsg,
}

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
