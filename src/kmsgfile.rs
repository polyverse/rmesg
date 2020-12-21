use crate::common;
use crate::entry::{Entry, EntryParsingError, EntryStruct};
/// This crate provides a klogctl interface from Rust.
/// klogctl is a Linux syscall that allows reading the Linux Kernel Log buffer.
/// https://elinux.org/Debugging_by_printing
///
/// This is a crate/library version of the popular Linux utility 'dmesg'
/// https://en.wikipedia.org/wiki/Dmesg
///
/// This allows Rust programs to consume dmesg-like output programmatically.
///
use crate::error::RMesgError;

use nonblock::NonBlockingReader;
use regex::Regex;
use std::fs;
use std::time::Duration;

/// suggest polling every ten seconds
pub const SUGGESTED_POLL_INTERVAL: std::time::Duration = Duration::from_secs(10);

#[cfg(feature = "async")]
use core::future::Future;
#[cfg(feature = "async")]
use core::pin::Pin;
#[cfg(feature = "async")]
use futures::stream::Stream;
#[cfg(feature = "async")]
use futures::task::{Context, Poll};
#[cfg(feature = "async")]
use tokio::time::{sleep, Sleep};

#[cfg(not(feature = "async"))]
use std::iter::Iterator;

const DEV_KMSG_PATH: &str = "/dev/kmsg";

pub fn kmsg_raw(file_override: Option<String>) -> Result<String, RMesgError> {
    let path = file_override.unwrap_or_else(|| DEV_KMSG_PATH.to_owned());

    let file = match fs::File::open(path.clone()) {
        Ok(fc) => fc,
        Err(e) => {
            return Err(RMesgError::DevKMsgFileOpenError(format!(
                "Unable to open file {}: {}",
                path, e
            )))
        }
    };

    let mut noblock_file = NonBlockingReader::from_fd(file)?;

    let mut file_contents = String::new();
    match noblock_file.read_available_to_string(&mut file_contents) {
        Ok(_) => {}
        Err(e) => {
            return Err(RMesgError::DevKMsgFileOpenError(format!(
                "Unable to open file {}: {}",
                path, e
            )))
        }
    }

    Ok(file_contents)
}

/// This is the key safe function that makes the klogctl syslog call with parameters.
/// While the internally used function supports all klogctl parameters, this function
/// only provides one bool parameter which indicates whether the buffer is to be cleared
/// or not, after its contents have been read.
///
/// Note that this is a by-definition synchronous function. So it is available
/// whether or not "async" feature is enabled
///
pub fn kmsg(file_override: Option<String>) -> Result<Vec<Entry>, RMesgError> {
    let file_contents = kmsg_raw(file_override)?;

    let lines = file_contents.as_str().lines();

    let mut entries = Vec::<Entry>::new();
    for line in lines {
        entries.push(entry_from_line(line)?)
    }
    Ok(entries)
}

// Message spec: https://github.com/torvalds/linux/blob/master/Documentation/ABI/testing/dev-kmsg
// Parses a kernel log line that looks like this (we ignore lines wtihout the timestamp):
// 5,0,0,-;Linux version 4.14.131-linuxkit (root@6d384074ad24) (gcc version 8.3.0 (Alpine 8.3.0)) #1 SMP Fri Jul 19 12:31:17 UTC 2019
// 6,1,0,-;Command, line: BOOT_IMAGE=/boot/kernel console=ttyS0 console=ttyS1 page_poison=1 vsyscall=emulate panic=1 root=/dev/sr0 text
//  LINE2=foobar
//  LINE 3 = foobar ; with semicolon
// 6,2,0,-;x86/fpu: Supporting XSAVE feature 0x001: 'x87 floating point registers'
// 6,3,0,-,more,deets;x86/fpu: Supporting XSAVE; feature 0x002: 'SSE registers'
pub fn entry_from_line(line: &str) -> Result<Entry, EntryParsingError> {
    lazy_static! {
        static ref RE_ENTRY_WITH_TIMESTAMP: Regex = Regex::new(
            r"(?x)^
            [[:space:]]*(?P<faclevstr>[[:digit:]]*)[[:space:]]*,
            # Sequence is a 64-bit integer: https://www.kernel.org/doc/Documentation/ABI/testing/dev-kmsg
            [[:space:]]*(?P<sequencenum>[[:digit:]]*)[[:space:]]*,
            [[:space:]]*(?P<timestamp>[[:digit:]]*)[[:space:]]*,
            # Ignore everything until the semi-colon and then the semicolon
            [[^;]]*;
            (?P<message>.*)$"
        )
        .unwrap();
    }

    if line.trim() == "" {
        return Err(EntryParsingError::EmptyLine);
    }

    let (facility, level, sequence_num, timestamp_from_system_start, message) =
        if let Some(kmsgparts) = RE_ENTRY_WITH_TIMESTAMP.captures(&line) {
            let (facility, level) = match kmsgparts.name("faclevstr") {
                Some(faclevstr) => common::parse_favlecstr(faclevstr.as_str(), line)?,
                None => (None, None),
            };

            let sequence_num = match kmsgparts.name("sequencenum") {
                Some(sequencestr) => {
                    Some(common::parse_fragment::<usize>(sequencestr.as_str(), line)?)
                }
                None => None,
            };

            let timestamp_from_system_start = match kmsgparts.name("timestampstr") {
                Some(timestampstr) => {
                    common::parse_timestamp_microsecs(timestampstr.as_str(), line)?
                }
                None => None,
            };

            let message = kmsgparts["message"].to_owned();

            (
                facility,
                level,
                sequence_num,
                timestamp_from_system_start,
                message,
            )
        } else {
            (None, None, None, None, line.to_owned())
        };

    cfg_if::cfg_if! {
        if #[cfg(feature="ptr")] {
            Ok(Box::new(EntryStruct{
                facility,
                level,
                timestamp_from_system_start,
                message,
            }))
        } else {
            Ok(EntryStruct {
                facility,
                level,
                sequence_num,
                timestamp_from_system_start,
                message,
            })
        }
    }
}

/**********************************************************************************/
// Tests! Tests! Tests!

#[cfg(all(test, target_os = "linux"))]
mod test {
    use super::*;

    #[test]
    fn test_kmsg() {
        let entries = kmsg(None);
        assert!(entries.is_ok(), "Failed to call rmesg");
        assert!(!entries.unwrap().is_empty(), "Should have non-empty logs");
    }

    #[test]
    fn test_parse_serialize() {
        let line1 = " LINE2=foobar";
        let e1r = entry_from_line(line1);
        assert!(e1r.is_ok());
        let line1again = e1r.unwrap().to_kmsg_str();
        assert_eq!(line1, line1again);

        let line2 = "6,1,0,-;Command, line: BOOT_IMAGE=/boot/kernel console=ttyS0 console=ttyS1 page_poison=1 vsyscall=emulate panic=1 root=/dev/sr0 text";
        let e2r = entry_from_line(line2);
        assert!(e2r.is_ok());
        let line2again = e2r.unwrap().to_kmsg_str();
        assert_eq!(line2, line2again);
    }
}
