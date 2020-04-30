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

// suggest polling every ten seconds
pub const SUGGESTED_POLL_INTERVAL: std::time::Duration = Duration::from_secs(10);

#[cfg(target_os = "linux")]
// Can be removed once upstream libc supports it.
extern "C" {
    fn klogctl(syslog_type: libc::c_int, buf: *mut libc::c_char, len: libc::c_int) -> libc::c_int;
}

#[cfg(not(target_os = "linux"))]
fn klogctl(_syslog_type: libc::c_int, _buf: *mut libc::c_char, _len: libc::c_int) -> libc::c_int {
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

pub const PRINTK_INCLUDE_TIMESTAMP_PARAMETER: &str = "/sys/module/printk/parameters/time";

pub struct RMesgLinesIterator {
    clear: bool,
    lines: Vec<String>,
    poll_interval: Duration,
    sleep_interval: Duration, // Just slightly longer than poll interval so the check passes
    last_poll: SystemTime,
    last_timestamp: f64,
}

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

pub fn kernel_log_timestamps_enabled() -> Result<bool, RMesgError> {
    Ok(fs::read_to_string(PRINTK_INCLUDE_TIMESTAMP_PARAMETER)?
        .trim()
        .to_uppercase()
        == "Y")
}

pub fn kernel_log_timestamps_enable(desired: bool) -> Result<(), RMesgError> {
    Ok(fs::write(
        PRINTK_INCLUDE_TIMESTAMP_PARAMETER,
        match desired {
            true => "Y\n",
            false => "N\n",
        },
    )?)
}

// ************************** Private

/*
    Safely wraps the klogctl for Rusty types
*/
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

#[cfg(test)]
mod test {
    use super::*;

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
}
