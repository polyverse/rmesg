use crate::common;
use crate::entry::{Entry, EntryParsingError};
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

use errno::errno;
use lazy_static::lazy_static;
use regex::Regex;
use std::convert::TryFrom;
use std::fs;
use std::time::{Duration, SystemTime};
use strum_macros::Display;

#[cfg(feature = "async")]
use core::future::Future;
#[cfg(feature = "async")]
use core::pin::Pin;
#[cfg(feature = "async")]
use futures::stream::Stream;
#[cfg(feature = "async")]
use futures::task::{Context, Poll};
#[cfg(feature = "async")]
use tokio::time as tokiotime;

#[cfg(feature = "sync")]
use std::iter::Iterator;
#[cfg(feature = "sync")]
use std::thread;

#[cfg(target_os = "linux")]
// Can be removed once upstream libc supports it.
extern "C" {
    fn klogctl(syslog_type: libc::c_int, buf: *mut libc::c_char, len: libc::c_int) -> libc::c_int;
}

/// Mark this as unsafe to be compliant with the Linux/Libc variant
/// Allows compilation on non-linux platforms which can be useful for
/// portability of downstream tools without complex conditionals.
#[cfg(not(target_os = "linux"))]
unsafe fn klogctl(
    _syslog_type: libc::c_int,
    _buf: *mut libc::c_char,
    _len: libc::c_int,
) -> libc::c_int {
    -1
}

// SYSLOG constants
// https://linux.die.net/man/3/klogctl
#[derive(Debug, Display, Clone)]
pub enum KLogType {
    SyslogActionClose,
    SyslogActionOpen,
    SyslogActionRead,
    SyslogActionReadAll,
    SyslogActionReadClear,
    SyslogActionClear,
    SyslogActionConsoleOff,
    SyslogActionConsoleOn,
    SyslogActionConsoleLevel,
    SyslogActionSizeUnread,
    SyslogActionSizeBuffer,
}

pub type SignedInt = libc::c_int;

/// The path under /proc where the parameter to set (or unset) logging a timestamp resides
pub const SYS_MODULE_PRINTK_PARAMETERS_TIME: &str = "/sys/module/printk/parameters/time";

/// suggest polling every ten seconds
pub const SUGGESTED_POLL_INTERVAL: std::time::Duration = Duration::from_secs(10);

lazy_static! {
    static ref RE_ENTRY_WITH_TIMESTAMP: Regex = Regex::new(
        r"(?x)^
        [[:space:]]*<(?P<faclevstr>[[:digit:]]*)>
        [[:space:]]*([\[][[:space:]]*(?P<timestampstr>[[:digit:]]*\.[[:digit:]]*)[\]])?
        (?P<message>.*)
        $"
    )
    .unwrap();
}

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
pub struct KLogEntries {
    clear: bool,
    entries: Vec<Entry>,
    last_timestamp: Option<Duration>,
    poll_interval: Duration,
    sleep_interval: Duration, // Just slightly longer than poll interval so the check passes
    last_poll: SystemTime,

    #[cfg(feature = "async")]
    sleep_future: Option<Pin<Box<tokiotime::Sleep>>>,
}

impl KLogEntries {
    /// Create a new KLogEntries with two specific options
    /// `clear: bool` specifies Whether or not to clear the buffer after every read.
    /// `poll_interval: Duration` specifies the interval after which to poll the buffer for new lines
    ///
    /// Choice of these parameters affects how the iterator behaves significantly.
    ///
    /// When `clear` is set, the buffer is cleared after each read. This means other utilities
    /// on the system that may also be reading the buffer will miss lines/data as it may be
    /// cleared before they can read it. This is a destructive option provided for completeness.
    ///
    /// The poll interval determines how frequently KLogEntries polls for new content.
    /// If the poll interval is too short, the iterator will eat up resources for no benefit.
    /// If it is too long, then any lines that showed up and were purged between the two polls
    /// will be lost.
    ///
    /// This crate exports a constant `SUGGESTED_POLL_INTERVAL` which contains the recommended
    /// default when in doubt.
    ///
    pub fn with_options(clear: bool, poll_interval: Duration) -> Result<KLogEntries, RMesgError> {
        let sleep_interval = match poll_interval.checked_add(Duration::from_millis(200)) {
            Some(si) => si,
            None => return Err(RMesgError::UnableToAddDurationToSystemTime),
        };

        // set last poll in the past so it polls the first time
        let last_poll = match SystemTime::now().checked_sub(sleep_interval) {
            Some(lp) => lp,
            None => return Err(RMesgError::UnableToAddDurationToSystemTime),
        };

        Ok(KLogEntries {
            entries: Vec::new(),
            poll_interval,
            sleep_interval,
            last_poll,
            clear,
            last_timestamp: None,

            #[cfg(feature = "async")]
            sleep_future: None,
        })
    }

    /// This method conducts the actual polling of the log buffer.
    ///
    /// It tracks the timestamp of the last line buffered, and only adds lines
    /// that have timestamps greater than that.
    ///
    /// Any lines without a timestamp are ignored. It is upto consumers to ensure timestamps
    /// are set (possibly through the provided function `kernel_log_timestamps_enable`) before
    /// polling/iterating.
    ///
    fn poll(&mut self) -> Result<usize, RMesgError> {
        self.last_poll = SystemTime::now();

        let mut entries = klog(self.clear)?;
        let mut entriesadded: usize = 0;
        match self.last_timestamp {
            None => {
                entriesadded += entries.len();
                self.entries.append(&mut entries);
            }
            Some(last_timestamp) => {
                while !entries.is_empty() {
                    let entry = entries.remove(0);
                    let skip = match entry.timestamp_from_system_start {
                        // skip if entry timestamp is older than or equal to last timestamp
                        Some(timestamp) => timestamp <= last_timestamp,
                        // skip all without timestamp
                        None => true,
                    };

                    if !skip {
                        self.entries.push(entry);
                        entriesadded += 1;
                    }
                }
            }
        };

        if let Some(entry) = self.entries.last() {
            if entry.timestamp_from_system_start.is_some() {
                self.last_timestamp = entry.timestamp_from_system_start;
            }
        }

        Ok(entriesadded)
    }
}

/// Trait to iterate over lines of the kernel log buffer.
#[cfg(feature = "sync")]
impl Iterator for KLogEntries {
    type Item = Result<Entry, RMesgError>;

    /// This is a blocking call, and will use the calling thread to perform polling
    /// NOT a thread-safe method either. It is suggested this method be always
    /// blocked on to ensure no messages are missed.
    fn next(&mut self) -> Option<Self::Item> {
        while self.entries.is_empty() {
            let elapsed = match self.last_poll.elapsed() {
                Ok(duration) => duration,
                Err(e) => return Some(Err(RMesgError::UnableToObtainElapsedTime(e))),
            };

            // Poll once if entering next and time since last poll
            // is greater than interval
            // This prevents lots of calls to next from hitting the kernel.
            if elapsed >= self.poll_interval {
                // poll once anyway
                if let Err(e) = self.poll() {
                    return Some(Err(e));
                }
            } else {
                thread::sleep(self.sleep_interval);
            }
        }

        Some(Ok(self.entries.remove(0)))
    }
}

/// Trait to iterate over lines of the kernel log buffer.
#[cfg(feature = "async")]
impl Stream for KLogEntries {
    type Item = Result<Entry, RMesgError>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(mut sf) = self.sleep_future.take() {
            match Future::poll(sf.as_mut(), cx) {
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
                let sf = tokiotime::sleep(self.sleep_interval);
                let mut pinned_sf = Box::pin(sf);
                if Future::poll(pinned_sf.as_mut(), cx).is_pending() {
                    self.sleep_future = Some(pinned_sf);
                    return Poll::Pending;
                }
            }
        }

        Poll::Ready(Some(Ok(self.entries.remove(0))))
    }
}

/// This is the key safe function that makes the klogctl syslog call with parameters.
/// While the internally used function supports all klogctl parameters, this function
/// only provides one bool parameter which indicates whether the buffer is to be cleared
/// or not, after its contents have been read.
///
/// Note that this is a by-definition synchronous function. So it is available
/// whether or not "async" feature is enabled
///
pub fn klog_raw(clear: bool) -> Result<String, RMesgError> {
    let mut dummy_buffer: Vec<u8> = vec![0; 0];
    let kernel_buffer_size =
        safely_wrapped_klogctl(KLogType::SyslogActionSizeBuffer, &mut dummy_buffer)?;

    let klogtype = match clear {
        true => KLogType::SyslogActionReadClear,
        false => KLogType::SyslogActionReadAll,
    };

    let mut real_buffer: Vec<u8> = vec![0; kernel_buffer_size];
    let bytes_read = safely_wrapped_klogctl(klogtype, &mut real_buffer)?;

    //adjust buffer capacity to what was read
    real_buffer.resize(bytes_read, 0);
    let utf8_str = String::from_utf8(real_buffer)?;

    // if incremental,
    Ok(utf8_str)
}

/// This is the key safe function that makes the klogctl syslog call with parameters.
/// While the internally used function supports all klogctl parameters, this function
/// only provides one bool parameter which indicates whether the buffer is to be cleared
/// or not, after its contents have been read.
///
/// Note that this is a by-definition synchronous function. So it is available
/// whether or not "async" feature is enabled
///
pub fn klog(clear: bool) -> Result<Vec<Entry>, RMesgError> {
    let all_lines = klog_raw(clear)?;
    Ok(entries_from_lines(&all_lines)?)
}

/// This function checks whether or not timestamps are enabled in the Linux Kernel log entries.
pub fn klog_timestamps_enabled() -> Result<bool, RMesgError> {
    Ok(fs::read_to_string(SYS_MODULE_PRINTK_PARAMETERS_TIME)?
        .trim()
        .to_uppercase()
        == "Y")
}

/// This function can enable or disable whether or not timestamps are enabled in the Linux Kernel log entries.
pub fn klog_timestamps_enable(desired: bool) -> Result<(), RMesgError> {
    Ok(fs::write(
        SYS_MODULE_PRINTK_PARAMETERS_TIME,
        match desired {
            true => "Y\n",
            false => "N\n",
        },
    )?)
}

// Message spec: https://github.com/torvalds/linux/blob/master/Documentation/ABI/testing/dev-kmsg
// Parses a kernel log line that looks like this (we ignore lines wtihout the timestamp):
// <5>a.out[4054]: segfault at 7ffd5503d358 ip 00007ffd5503d358 sp 00007ffd5503d258 error 15
// OR
// <5>[   233434.343533] a.out[4054]: segfault at 7ffd5503d358 ip 00007ffd5503d358 sp 00007ffd5503d258 error 15
pub fn entries_from_lines(all_lines: &str) -> Result<Vec<Entry>, EntryParsingError> {
    let entry_results: Result<Vec<Entry>, EntryParsingError> = all_lines
        .lines()
        .map(|line| entry_from_line(line))
        .collect();

    Ok(entry_results?)
}

pub fn entry_from_line(line: &str) -> Result<Entry, EntryParsingError> {
    if let Some(klogparts) = RE_ENTRY_WITH_TIMESTAMP.captures(line) {
        let (facility, level) = match klogparts.name("faclevstr") {
            Some(faclevstr) => common::parse_favlecstr(faclevstr.as_str(), line)?,
            None => (None, None),
        };

        let timestamp_from_system_start = match klogparts.name("timestampstr") {
            Some(timestampstr) => common::parse_timestamp_secs(timestampstr.as_str(), line)?,
            None => None,
        };

        let message = klogparts["message"].to_owned();

        Ok(Entry {
            facility,
            level,
            sequence_num: None,
            timestamp_from_system_start,
            message,
        })
    } else {
        Ok(Entry {
            facility: None,
            level: None,
            sequence_num: None,
            timestamp_from_system_start: None,
            message: line.to_owned(),
        })
    }
}

// ************************** Private

/// Safely wraps the klogctl for Rusty types
/// All higher-level functions are built over this function at the base.
/// It prevents unsafe code from proliferating beyond this wrapper.
pub fn safely_wrapped_klogctl(klogtype: KLogType, buf_u8: &mut [u8]) -> Result<usize, RMesgError> {
    // convert klogtype
    let klt = klogtype.clone() as libc::c_int;

    // extract mutable u8 raw pointer from buf
    // and typecast it (very dangerously) to i8
    // fortunately it's all one-byte long so
    // should be reasonably okay.
    let buf_cchar = buf_u8.as_mut_ptr() as *mut libc::c_char;

    let buflen = match libc::c_int::try_from(buf_u8.len()) {
        Ok(i) => i,
        Err(e) => {
            return Err(RMesgError::IntegerOutOfBound(format!(
                "Error converting buffer length for klogctl from <usize>::({}) into <c_int>: {:?}",
                buf_u8.len(),
                e
            )))
        }
    };

    let response_cint: libc::c_int = unsafe { klogctl(klt, buf_cchar, buflen) };

    if response_cint < 0 {
        let err = errno();
        return Err(RMesgError::InternalError(format!(
            "Request ({}) to klogctl failed. errno={}",
            klogtype, err
        )));
    }

    let response = match usize::try_from(response_cint) {
        Ok(i) => i,
        Err(e) => {
            return Err(RMesgError::IntegerOutOfBound(format!(
                "Error converting response from klogctl from <c_int>::({}) into <usize>: {:?}",
                response_cint, e
            )))
        }
    };

    Ok(response)
}

/**********************************************************************************/
// Tests! Tests! Tests!

#[cfg(all(test, target_os = "linux"))]
mod test {
    use super::*;

    #[test]
    fn get_kernel_buffer_size() {
        let mut dummy_buffer: Vec<u8> = vec![0; 0];
        let response = safely_wrapped_klogctl(KLogType::SyslogActionSizeBuffer, &mut dummy_buffer);
        assert!(response.is_ok(), "Response from klogctl not Ok");
        assert!(
            response.unwrap() > 0,
            "Buffer size should be greater than zero."
        );
    }

    #[test]
    fn test_klog() {
        let entries = klog(false);
        assert!(entries.is_ok(), "Response from klog not Ok");
        assert!(!entries.unwrap().is_empty(), "Should have non-empty logs");
    }

    #[cfg(feature = "sync")]
    #[test]
    fn test_iterator() {
        // uncomment below if you want to be extra-sure
        //let enable_timestamp_result = kernel_log_timestamps_enable(true);
        //assert!(enable_timestamp_result.is_ok());

        // Don't clear the buffer. Poll every second.
        let iterator_result = KLogEntries::with_options(false, SUGGESTED_POLL_INTERVAL);
        assert!(iterator_result.is_ok());

        let iterator = iterator_result.unwrap();

        // Read 10 lines and quit
        for (count, entry) in iterator.enumerate() {
            assert!(entry.is_ok());
            if count > 10 {
                break;
            }
        }
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_stream() {
        // uncomment below if you want to be extra-sure
        //let enable_timestamp_result = kernel_log_timestamps_enable(true);
        //assert!(enable_timestamp_result.is_ok());

        // Don't clear the buffer. Poll every second.
        let stream_result = KLogEntries::with_options(false, SUGGESTED_POLL_INTERVAL);
        assert!(stream_result.is_ok());

        let mut stream = stream_result.unwrap();

        // Read 10 lines and quit
        let mut count: u32 = 0;
        while let Some(entry) = tokio_stream::StreamExt::next(&mut stream).await {
            assert!(entry.is_ok());
            count += 1;
            if count > 10 {
                break;
            }
        }
    }

    #[test]
    fn test_parse_serialize() {
        let line1 = "<6>a.out[4054]: segfault at 7ffd5503d358 ip 00007ffd5503d358 sp 00007ffd5503d258 error 15";
        let entries1 = entries_from_lines(line1).unwrap();
        let e1r = entries1.get(0).unwrap();
        let line1again = e1r.to_klog_str().unwrap();
        assert_eq!(line1, line1again);

        let line2 = "<7>[   233434.343533] a.out[4054]: segfault at 7ffd5503d358 ip 00007ffd5503d358 sp 00007ffd5503d258 error 15";
        let entries2 = entries_from_lines(line2).unwrap();
        let e2r = entries2.get(0).unwrap();
        let line2again = e2r.to_klog_str().unwrap();
        assert_eq!(line2, line2again);

        let line3 = "233434.343533] a.out[4054]: segfault at 7ffd5503d358 ip 00007ffd5503d358 sp 00007ffd5503d258 error 15";
        let entries3 = entries_from_lines(line3).unwrap();
        let e3r = entries3.get(0).unwrap();
        let line3again = e3r.to_klog_str().unwrap();
        assert_eq!(line3, line3again);
    }

    #[test]
    fn test_parse_multiline() {
        let line1 = "<6>a.out[4054]: segfault at 7ffd5503d358 ip 00007ffd5503d358 sp 00007ffd5503d258 error 15";
        let line2 = "<7>[   233434.343533] a.out[4054]: segfault at 7ffd5503d358 ip 00007ffd5503d358 sp 00007ffd5503d258 error 15";
        let line3 = "233434.343533] a.out[4054]: segfault at 7ffd5503d358 ip 00007ffd5503d358 sp 00007ffd5503d258 error 15";

        let lines = [line1, line2, line3].join("\n");

        let mut entries = entries_from_lines(&lines).unwrap();

        let e1r = entries.remove(0);
        let line1again = e1r.to_klog_str().unwrap();
        assert_eq!(line1, line1again);

        let e2r = entries.remove(0);
        let line2again = e2r.to_klog_str().unwrap();
        assert_eq!(line2, line2again);

        let e3r = entries.remove(0);
        let line3again = e3r.to_klog_str().unwrap();
        assert_eq!(line3, line3again);
    }
}
