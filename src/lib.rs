#[cfg(target_os = "linux")]
#[macro_use]
extern crate enum_display_derive;

pub mod error;

use error::RMesgError;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

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

// suggest polling every ten seconds
pub const SUGGESTED_POLL_INTERVAL: std::time::Duration = Duration::from_secs(10);

pub struct RMesgLinesIterator {
    clear: bool,
    lines: Vec<String>,
    poll_interval: Duration,
    sleep_interval: Duration, // Just slightly longer than poll interval so the check passes
    last_poll: SystemTime,
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
    fn poll(&mut self) -> Result<(), RMesgError> {
        let rawlogs = rmesg(self.clear)?;
        for line in rawlogs.lines() {
            self.lines.push(line.to_owned());
        }

        Ok(())
    }
}

pub fn rmesg_lines_iter(
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
    })
}
