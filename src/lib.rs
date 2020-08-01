/// This crate provides a klogctl interface from Rust.
/// klogctl is a Linux syscall that allows reading the Linux Kernel Log buffer.
/// https://elinux.org/Debugging_by_printing
///
/// This is a crate/library version of the popular Linux utility 'dmesg'
/// https://en.wikipedia.org/wiki/Dmesg
///
/// This allows Rust programs to consume dmesg-like output programmatically.
///

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate enum_display_derive;

pub mod error;

use errno::errno;
use error::RMesgError;
use libc;
use regex::Regex;
use std::convert::TryFrom;
use std::fmt::Display;
use std::fs;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

/// suggest polling every ten seconds
pub const SUGGESTED_POLL_INTERVAL: std::time::Duration = Duration::from_secs(10);

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
    return -1;
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

pub const SYS_MODULE_PRINTK_PARAMETERS_TIME: &str = "/sys/module/printk/parameters/time";

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
pub struct RMesgLinesIterator {
    clear: bool,
    lines: Vec<String>,
    poll_interval: Duration,
    sleep_interval: Duration, // Just slightly longer than poll interval so the check passes
    last_poll: SystemTime,
    last_timestamp: f64,
}

/// Trait to iterate over lines of the kernel log buffer.
impl std::iter::Iterator for RMesgLinesIterator {
    type Item = Result<String, RMesgError>;

    /// This is a blocking call, and will use the calling thread to perform polling
    /// NOT a thread-safe method either. It is suggested this method be always
    /// blocked on to ensure no messages are missed.
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let elapsed = match self.last_poll.elapsed() {
                Ok(duration) => duration,
                Err(e) => {
                    eprintln!(
                        "Error occurred when obtaining elapsed time since last poll: {:?}",
                        e
                    );
                    return None;
                }
            };
            // Poll once if entering next and time since last poll
            // is greater than interval
            // This prevents lots of calls to next from hitting the kernel.
            if elapsed >= self.poll_interval {
                // poll once anyway
                if let Err(e) = self.poll() {
                    eprintln!(
                        "An error occurred when polling rmesg for new messages to trail: {}",
                        e
                    );
                    return None;
                }
            }

            if self.lines.len() == 0 {
                // sleep for poll duration, then loop
                sleep(self.sleep_interval);

                // loop over
                continue;
            }

            return Some(Ok(self.lines.remove(0)));
        }
    }
}

impl RMesgLinesIterator {
    /// Create a new RMesgLinesIterator with two specific options
    /// `clear: bool` specifies Whether or not to clear the buffer after every read.
    /// `poll_interval: Duration` specifies the interval after which to poll the buffer for new lines
    ///
    /// Choice of these parameters affects how the iterator behaves significantly.
    ///
    /// When `clear` is set, the buffer is cleared after each read. This means other utilities
    /// on the system that may also be reading the buffer will miss lines/data as it may be
    /// cleared before they can read it. This is a destructive option provided for completeness.
    ///
    /// The poll interval determines how frequently RMesgLinesIterator polls for new content.
    /// If the poll interval is too short, the iterator will eat up resources for no benefit.
    /// If it is too long, then any lines that showed up and were purged between the two polls
    /// will be lost.
    ///
    /// This crate exports a constant `SUGGESTED_POLL_INTERVAL` which contains the recommended
    /// default when in doubt.
    ///
    pub fn with_options(
        clear: bool,
        poll_interval: Duration,
    ) -> Result<RMesgLinesIterator, RMesgError> {
        let sleep_interval = match poll_interval.checked_add(Duration::from_millis(200)) {
            Some(si) => si,
            None => return Err(RMesgError::UnableToAddDurationToSystemTime),
        };

        let last_poll = match SystemTime::now().checked_sub(sleep_interval) {
            Some(lp) => lp,
            None => return Err(RMesgError::UnableToAddDurationToSystemTime),
        };

        Ok(RMesgLinesIterator {
            // Give it a thousand-line capacity vector to start
            lines: Vec::with_capacity(1000),
            poll_interval,
            sleep_interval,
            // set last poll in the past so it polls the first time
            last_poll,
            clear,
            last_timestamp: -1.0, //start negative so all zero-timestamped events are picked up
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
        let rawlogs = rmesg(self.clear)?;

        let last_timestamp = self.last_timestamp;
        let newer_lines = rawlogs
            .lines()
            .filter_map(RMesgLinesIterator::extract_timestamp)
            .skip_while(|(timestamp, _)| timestamp <= &last_timestamp);

        let mut linesadded: usize = 0;
        for (timestamp, newline) in newer_lines {
            self.lines.push(newline.to_owned());
            linesadded = linesadded + 1;
            self.last_timestamp = timestamp;
        }

        return Ok(linesadded);
    }

    /// Extracts a timestamp in the log line (if one exists) and returns
    /// it or None.
    fn extract_timestamp(line: &str) -> Option<(f64, &str)> {
        lazy_static! {
            static ref RE_RMESG_WITH_TIMESTAMP: Regex = Regex::new(
                r"(?x)^
                [^\[]*[\[][[:space:]]*(?P<timestampstr>[[:xdigit:]]*\.[[:xdigit:]]*)[\]]
                .*$"
            )
            .unwrap();
        }
        if let Some(rmesgparts) = RE_RMESG_WITH_TIMESTAMP.captures(line) {
            match rmesgparts["timestampstr"].parse::<f64>() {
                Ok(timesecs) => Some((timesecs, line)),
                Err(_) => None,
            }
        } else {
            None
        }
    }
}

/// This is the key safe function that makes the klogctl syslog call with parameters.
/// While the internally used function supports all klogctl parameters, this function
/// only provides one bool parameter which indicates whether the buffer is to be cleared
/// or not, after its contents have been read.
pub fn rmesg(clear: bool) -> Result<String, RMesgError> {
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

/// This function checks whether or not timestamps are enabled in the Linux Kernel log entries.
pub fn kernel_log_timestamps_enabled() -> Result<bool, RMesgError> {
    Ok(fs::read_to_string(SYS_MODULE_PRINTK_PARAMETERS_TIME)?
        .trim()
        .to_uppercase()
        == "Y")
}

/// This function can enable or disable whether or not timestamps are enabled in the Linux Kernel log entries.
pub fn kernel_log_timestamps_enable(desired: bool) -> Result<(), RMesgError> {
    Ok(fs::write(
        SYS_MODULE_PRINTK_PARAMETERS_TIME,
        match desired {
            true => "Y\n",
            false => "N\n",
        },
    )?)
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
    let buf_i8 = buf_u8.as_mut_ptr() as *mut i8;

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

    let response_cint: libc::c_int = unsafe { klogctl(klt, buf_i8, buflen) };

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

    return Ok(response);
}

/**********************************************************************************/
// Tests! Tests! Tests!

#[cfg(all(test, target_os = "linux"))]
mod test {
    use super::{
        kernel_log_timestamps_enable, rmesg, safely_wrapped_klogctl, KLogType, RMesgLinesIterator,
        SUGGESTED_POLL_INTERVAL,
    };

    #[test]
    fn get_kernel_buffer_size() {
        let mut dummy_buffer: Vec<u8> = vec![0; 0];
        let response = safely_wrapped_klogctl(KLogType::SyslogActionSizeBuffer, &mut dummy_buffer);
        assert!(response.is_ok(), "Failed to call klogctl");
        assert!(
            response.unwrap() > 0,
            "Buffer size should be greater than zero."
        );
    }

    #[test]
    fn test_rmesg() {
        let logs = rmesg(false);
        assert!(logs.is_ok(), "Failed to call rmesg");
        assert!(logs.unwrap().len() > 0, "Should have non-empty logs");
    }

    #[test]
    fn test_iterator() {
        let enable_timestamp_result = kernel_log_timestamps_enable(true);
        assert!(enable_timestamp_result.is_ok());

        // Don't clear the buffer. Poll every second.
        let iterator_result = RMesgLinesIterator::with_options(false, SUGGESTED_POLL_INTERVAL);
        assert!(iterator_result.is_ok());

        let iterator = iterator_result.unwrap();

        // Read 10 lines and quit
        let mut count: u32 = 0;
        for line in iterator {
            assert!(line.is_ok());
            count = count + 1;
            if count > 10 {
                break;
            }
        }
    }
}
