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
use std::io::{BufReader, Lines, BufRead};

#[cfg(not(feature="async"))]
use std::iter::Iterator;

const DEV_KMSG_PATH: &str = "/dev/kmsg";

/// While reading the kernel log buffer is very useful in and of itself (expecially when running the CLI),
/// a lot more value is unlocked when it can be tailed line-by-line.
///
/// This struct provides the facilities to do that. It implements an iterator to easily iterate
/// indefinitely over the lines.
///
/// IMPORTANT NOTE: This iterator makes a best-effort attempt at eliminating duplicate lines
/// so that it can only provide newer lines upon each iteration. The way it accomplishes this is
/// by using the timestamp field, to track the last-seen timestamp of a line already buffered,
/// and this only consuming lines past that timestamp on each poll.
///
/// The timestamp may not always be set in kernel logs. The iterator will ignore lines without a timestamp.
/// It is left to the consumers of this struct to ensure the timestamp is set, if they wish for
/// lines to not be ignored. In order to aid this, two functions are provided in this crate to
/// check `kernel_log_timestamps_enabled` and to set or unset `kernel_log_timestamps_enable`.
///
/// The UX is left to the consumer.
///
pub struct KMsgEntries {
    raw: bool,
    lines: Lines<BufReader<fs::File>>,
}

impl KMsgEntries {
    /// Create a new KMsgEntries with two specific options
    /// `clear: bool` specifies Whether or not to clear the buffer after every read.
    /// `poll_interval: Duration` specifies the interval after which to poll the buffer for new lines
    ///
    /// Choice of these parameters affects how the iterator behaves significantly.
    ///
    /// When `clear` is set, the buffer is cleared after each read. This means other utilities
    /// on the system that may also be reading the buffer will miss lines/data as it may be
    /// cleared before they can read it. This is a destructive option provided for completeness.
    ///
    /// The poll interval determines how frequently KMsgEntries polls for new content.
    /// If the poll interval is too short, the iterator will eat up resources for no benefit.
    /// If it is too long, then any lines that showed up and were purged between the two polls
    /// will be lost.
    ///
    /// This crate exports a constant `SUGGESTED_POLL_INTERVAL` which contains the recommended
    /// default when in doubt.
    ///
    pub fn with_options(file_override: Option<String>, raw: bool) -> Result<KMsgEntries, RMesgError> {
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

        let lines = BufReader::new(file).lines();

        Ok(KMsgEntries {
            raw,
            lines,
        })
    }
}

/// Trait to iterate over lines of the kernel log buffer.
#[cfg(not(feature = "async"))]
impl Iterator for KMsgEntries {
    type Item = Result<Entry, RMesgError>;

    /// This is a blocking call, and will use the calling thread to perform polling
    /// NOT a thread-safe method either. It is suggested this method be always
    /// blocked on to ensure no messages are missed.
    fn next(&mut self) -> Option<Self::Item> {
        match self.lines.next() {
            None => None,
            Some(Err(e)) => Some(Err(RMesgError::IOError(format!("Error reading next line from kernel log device file: {}", e)))),
            Some(Ok(line)) => {
                if self.raw {
                    let entry = EntryStruct{
                        facility: None,
                        level: None,
                        timestamp_from_system_start: None,
                        sequence_num: None,
                        message: line,
                    };

                    cfg_if::cfg_if! {
                        if #[cfg(feature="ptr")] {
                            Some(Ok(Box::new(entry)))
                        } else {
                            Some(Ok(entry))
                        }
                    }
                } else {
                    Some(entry_from_line(&line).map_err(|e| e.into()))
                }
            },
        }
    }
}

/// Trait to iterate over lines of the kernel log buffer.
#[cfg(feature = "async")]
impl Stream for KMsgEntries {
    type Item = Result<Entry, RMesgError>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(mut sf) = self.sleep_future.take() {
            match Future::poll(Pin::new(&mut sf), cx) {
                // still sleeping? Go back to sleep.
                Poll::Pending => {
                    // put the future back in
                    self.sleep_future = Some(sf);
                    return Poll::Pending;
                }

                // Not sleeping?
                Poll::Ready(()) => {}
            }
        }

        // entries empty?
        while self.entries.is_empty() {
            let elapsed = match self.last_poll.elapsed() {
                Ok(duration) => duration,
                Err(e) => return Poll::Ready(Some(Err(RMesgError::UnableToObtainElapsedTime(e)))),
            };

            // Did enough time pass since last poll? If so try to poll
            if elapsed >= self.poll_interval {
                if let Err(e) = self.poll() {
                    return Poll::Ready(Some(Err(e)));
                }
            } else {
                let mut sf = sleep(self.sleep_interval);
                if let Poll::Pending = Future::poll(Pin::new(&mut sf), cx) {
                    self.sleep_future = Some(sf);
                    return Poll::Pending;
                }
            }
        }

        Poll::Ready(Some(Ok(self.entries.remove(0))))
    }
}

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
            [[:space:]]*(?P<timestampstr>[[:digit:]]*)[[:space:]]*,
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

        let entry = EntryStruct{
            facility,
            level,
            sequence_num,
            timestamp_from_system_start,
            message,
        };

        cfg_if::cfg_if! {
            if #[cfg(feature="ptr")] {
                Ok(Box::new(entry))
            } else {
                Ok(entry)
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

        let line2 = "6,779,91650777797,-;docker0: port 2(veth98d5024) entered disabled state";
        let e2r = entry_from_line(line2);
        assert!(e2r.is_ok());
        let line2again = e2r.unwrap().to_kmsg_str();
        assert_eq!(line2, line2again);
    }
}
