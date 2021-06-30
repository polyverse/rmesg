mod common;

pub mod entry;
pub mod error;
/// KLog Implementation (makes klogctl aka syslog system call through libc)
pub mod klogctl;
/// KMsg Implementation (reads from the /dev/kmsg file)
pub mod kmsgfile;

#[cfg(feature = "sync")]
use std::iter::Iterator;

#[cfg(feature = "async")]
use core::pin::Pin;
#[cfg(feature = "async")]
use futures::stream::Stream;
#[cfg(feature = "async")]
use futures::task::{Context, Poll};
#[cfg(feature = "async")]
use pin_project::pin_project;

#[derive(Clone, Copy, Debug)]
pub enum Backend {
    Default,
    KLogCtl,
    DevKMsg,
}

#[cfg(feature = "sync")]
pub enum EntriesIterator {
    KLogCtl(klogctl::KLogEntries),
    DevKMsg(kmsgfile::KMsgEntriesIter),
}
#[cfg(feature = "sync")]
impl Iterator for EntriesIterator {
    type Item = Result<entry::Entry, error::RMesgError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::KLogCtl(k) => k.next(),
            Self::DevKMsg(d) => d.next(),
        }
    }
}

#[pin_project(project = EntriesStreamPinnedProjection)]
#[cfg(feature = "async")]
pub enum EntriesStream {
    KLogCtl(#[pin] klogctl::KLogEntries),
    DevKMsg(#[pin] kmsgfile::KMsgEntriesStream),
}
#[cfg(feature = "async")]
impl Stream for EntriesStream {
    type Item = Result<entry::Entry, error::RMesgError>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project() {
            EntriesStreamPinnedProjection::KLogCtl(k) => k.poll_next(cx),
            EntriesStreamPinnedProjection::DevKMsg(d) => d.poll_next(cx),
        }
    }
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
                if std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) {
                    eprintln!("Help: run rmesg with sudo");
                    return Ok(vec![]);
                }
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

#[cfg(feature = "sync")]
pub fn logs_iter(b: Backend, clear: bool, raw: bool) -> Result<EntriesIterator, error::RMesgError> {
    match b {
        Backend::Default => match kmsgfile::KMsgEntriesIter::with_options(None, raw) {
            Ok(e) => Ok(EntriesIterator::DevKMsg(e)),
            Err(error::RMesgError::DevKMsgFileOpenError(s)) => {
                eprintln!(
                    "Falling back from device file to klogctl syscall due to error: {}",
                    s
                );
                Ok(EntriesIterator::KLogCtl(
                    klog_entries_only_if_timestamp_enabled(clear)?,
                ))
            }
            Err(e) => Err(e),
        },
        Backend::KLogCtl => Ok(EntriesIterator::KLogCtl(
            klog_entries_only_if_timestamp_enabled(clear)?,
        )),
        Backend::DevKMsg => Ok(EntriesIterator::DevKMsg(
            kmsgfile::KMsgEntriesIter::with_options(None, raw)?,
        )),
    }
}

#[cfg(feature = "async")]
pub async fn logs_stream(
    b: Backend,
    clear: bool,
    raw: bool,
) -> Result<EntriesStream, error::RMesgError> {
    match b {
        Backend::Default => match kmsgfile::KMsgEntriesStream::with_options(None, raw).await {
            Ok(e) => Ok(EntriesStream::DevKMsg(e)),
            Err(error::RMesgError::DevKMsgFileOpenError(s)) => {
                eprintln!(
                    "Falling back from device file to klogctl syscall due to error: {}",
                    s
                );
                Ok(EntriesStream::KLogCtl(
                    klog_entries_only_if_timestamp_enabled(clear)?,
                ))
            }
            Err(e) => Err(e),
        },
        Backend::KLogCtl => Ok(EntriesStream::KLogCtl(
            klog_entries_only_if_timestamp_enabled(clear)?,
        )),
        Backend::DevKMsg => Ok(EntriesStream::DevKMsg(
            kmsgfile::KMsgEntriesStream::with_options(None, raw).await?,
        )),
    }
}

fn klog_entries_only_if_timestamp_enabled(
    clear: bool,
) -> Result<klogctl::KLogEntries, error::RMesgError> {
    let log_timestamps_enabled = klogctl::klog_timestamps_enabled()?;

    // ensure timestamps in logs
    if !log_timestamps_enabled {
        eprintln!("WARNING: Timestamps are disabled but tailing/following logs (as you've requested) requires them.");
        eprintln!("Aboring program.");
        eprintln!("You can enable timestamps by running the following: ");
        eprintln!("  echo Y > /sys/module/printk/parameters/time");
        return Err(error::RMesgError::KLogTimestampsDisabled);
    }

    klogctl::KLogEntries::with_options(clear, klogctl::SUGGESTED_POLL_INTERVAL)
}

/**********************************************************************************/
// Tests! Tests! Tests!

#[cfg(all(test, target_os = "linux"))]
mod test {
    use super::*;
    #[cfg(feature = "async")]
    use tokio_stream::StreamExt;

    #[test]
    fn test_log_entries() {
        let entries = log_entries(Backend::Default, false);
        assert!(entries.is_ok(), "Response from kmsg not Ok");
        assert!(!entries.unwrap().is_empty(), "Should have non-empty logs");
    }

    #[cfg(feature = "sync")]
    #[test]
    fn test_iterator() {
        // uncomment below if you want to be extra-sure
        //let enable_timestamp_result = kernel_log_timestamps_enable(true);
        //assert!(enable_timestamp_result.is_ok());

        // Don't clear the buffer. Poll every second.
        let iterator_result = logs_iter(Backend::Default, false, false);
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
        let stream_result = logs_stream(Backend::Default, false, false).await;
        assert!(stream_result.is_ok());

        let mut stream = stream_result.unwrap();

        // Read 10 lines and quit
        let mut count: u32 = 0;
        while let Some(entry) = stream.next().await {
            assert!(entry.is_ok());
            count += 1;
            if count > 10 {
                break;
            }
        }
    }
}
