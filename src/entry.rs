// Copyright (c) 2019 Polyverse Corporation

use num_derive::FromPrimitive;
use std::error::Error;
use std::fmt::{Display, Error as FmtError, Formatter, Result as FmtResult, Write};
use std::time::Duration;
use strum_macros::{Display, EnumString};

#[cfg(feature = "extra-traits")]
use serde::{Deserialize, Serialize};

/// A parsed/structured entry from kernel log buffer
#[derive(PartialEq, Debug, Clone)]
pub struct Entry {
    // Log facility
    pub facility: Option<LogFacility>,

    // Log level
    pub level: Option<LogLevel>,

    // Log sequence number
    pub sequence_num: Option<usize>,

    // The amount of time since system bootstrapped
    pub timestamp_from_system_start: Option<Duration>,

    // Log message
    pub message: String,
}

impl Entry {
    pub fn to_faclev(&self) -> Option<u8> {
        match (self.facility, self.level) {
            (Some(facility), Some(level)) => Some(((facility as u8) << 3) + (level as u8)),
            _ => None,
        }
    }

    // Like so:
    // <5>a.out[4054]: segfault at 7ffd5503d358 ip 00007ffd5503d358 sp 00007ffd5503d258 error 15
    // OR
    // <5>[   233434.343533] a.out[4054]: segfault at 7ffd5503d358 ip 00007ffd5503d358 sp 00007ffd5503d258 error 15
    pub fn to_klog_str(&self) -> Result<String, FmtError> {
        if let Some(faclev) = self.to_faclev() {
            // +6 for buffer + capacity is 16+6 (for timestamp) + 2 (for []) + 2 (for <>) + 1 for facllev + message
            let mut retstr = String::with_capacity(35 + self.message.len());

            write!(retstr, "<{}>", faclev)?;

            if let Some(ts) = self.timestamp_from_system_start {
                write!(retstr, "[{: >16.6}]", ts.as_secs_f64())?;
            }

            write!(retstr, "{}", self.message)?;

            Ok(retstr)
        } else {
            Ok(self.message.to_owned())
        }
    }

    // Like so:
    // 6,1,0,-;Command, line: BOOT_IMAGE=/boot/kernel console=ttyS0 console=ttyS1 page_poison=1 vsyscall=emulate panic=1 root=/dev/sr0 text
    //  LINE2=foobar
    //  LINE 3 = foobar ; with semicolon
    pub fn to_kmsg_str(&self) -> Result<String, FmtError> {
        if let Some(faclev) = self.to_faclev() {
            // +7 for buffer + capacity is 12 (for timestamp) + 5 (for punctuations) + 1 for facllev + message
            let mut retstr = String::with_capacity(25 + self.message.len());

            let sequence_num = self.sequence_num.unwrap_or(0);
            write!(retstr, "{},{},", faclev, sequence_num)?;

            if let Some(ts) = self.timestamp_from_system_start {
                write!(retstr, "{},-;", ts.as_micros())?;
            } else {
                retstr.push_str("0,-;");
            }

            write!(retstr, "{}", self.message)?;

            Ok(retstr)
        } else {
            Ok(self.message.to_string())
        }
    }
}

impl Display for Entry {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        if let Some(ts) = self.timestamp_from_system_start {
            write!(f, "[{: >16.6}] ", ts.as_secs_f64())?
        }

        write!(f, "{}", self.message)
    }
}

/// Linux kmesg (kernel message buffer) Log Facility.
#[cfg_attr(feature = "extra-traits", derive(Serialize, Deserialize))]
#[derive(EnumString, Debug, PartialEq, Display, Copy, Clone, FromPrimitive)]
pub enum LogFacility {
    #[strum(serialize = "kern")]
    Kern = 0,

    #[strum(serialize = "user")]
    User,

    #[strum(serialize = "mail")]
    Mail,

    #[strum(serialize = "daemon")]
    Daemon,

    #[strum(serialize = "auth")]
    Auth,

    #[strum(serialize = "syslog")]
    Syslog,

    #[strum(serialize = "lpr")]
    Lpr,

    #[strum(serialize = "news")]
    News,

    #[strum(serialize = "uucp")]
    UUCP,

    #[strum(serialize = "cron")]
    Cron,

    #[strum(serialize = "authpriv")]
    AuthPriv,

    #[strum(serialize = "ftp")]
    FTP,
}

/// Linux kmesg (kernel message buffer) Log Level.
#[cfg_attr(feature = "extra-traits", derive(Serialize, Deserialize))]
#[derive(EnumString, Debug, PartialEq, Display, Copy, Clone, FromPrimitive)]
pub enum LogLevel {
    #[strum(serialize = "emerg")]
    Emergency = 0,

    #[strum(serialize = "alert")]
    Alert,

    #[strum(serialize = "crit")]
    Critical,

    #[strum(serialize = "err")]
    Error,

    #[strum(serialize = "warn")]
    Warning,

    #[strum(serialize = "notice")]
    Notice,

    #[strum(serialize = "info")]
    Info,

    #[strum(serialize = "debug")]
    Debug,
}

#[derive(Debug)]
pub enum EntryParsingError {
    Completed,
    EventTooOld,
    EmptyLine,
    Generic(String),
}
impl Error for EntryParsingError {}
impl Display for EntryParsingError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "KMsgParsingError:: {}",
            match self {
                Self::Completed => "Completed Parsing",
                Self::EventTooOld =>
                    "Event too old due to timestamp or sequence number (we've parsed newer messages than these)",
                    Self::EmptyLine => "Empty line",
                    Self::Generic(s) => s,
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_to_klog() {
        let entry_struct = Entry {
            timestamp_from_system_start: Some(Duration::from_secs_f64(24241.325252)),
            facility: Some(LogFacility::Kern),
            level: Some(LogLevel::Info),
            sequence_num: Some(10),
            message: "Test message".to_owned(),
        };
        let expected_serialization = "<6>[    24241.325252]Test message";

        let boxed_entry_struct = Box::new(entry_struct.clone());

        let printed_entry_struct = entry_struct.to_klog_str().unwrap();
        assert_eq!(printed_entry_struct, expected_serialization);

        let printed_boxed_entry_struct = boxed_entry_struct.to_klog_str().unwrap();
        assert_eq!(printed_boxed_entry_struct, expected_serialization);
    }

    #[test]
    fn test_serialize_to_kmsg() {
        let entry_struct = Entry {
            timestamp_from_system_start: Some(Duration::from_secs_f64(24241.325252)),
            facility: Some(LogFacility::Kern),
            level: Some(LogLevel::Info),
            sequence_num: Some(23),
            message: "Test message".to_owned(),
        };
        let expected_serialization = "6,23,24241325252,-;Test message";

        let boxed_entry_struct = Box::new(entry_struct.clone());

        let printed_entry_struct = entry_struct.to_kmsg_str().unwrap();
        assert_eq!(printed_entry_struct, expected_serialization);

        let printed_boxed_entry_struct = boxed_entry_struct.to_kmsg_str().unwrap();
        assert_eq!(printed_boxed_entry_struct, expected_serialization);
    }

    #[test]
    fn test_display() {
        let entry_struct = Entry {
            timestamp_from_system_start: Some(Duration::from_secs_f64(24241.325252)),
            facility: Some(LogFacility::Kern),
            level: Some(LogLevel::Info),
            sequence_num: Some(15),
            message: "Test message".to_owned(),
        };
        let expected_serialization = "[    24241.325252] Test message";

        let boxed_entry_struct = Box::new(entry_struct.clone());

        let printed_entry_struct = format!("{}", entry_struct);
        assert_eq!(printed_entry_struct, expected_serialization);

        let printed_boxed_entry_struct = format!("{}", boxed_entry_struct);
        assert_eq!(printed_boxed_entry_struct, expected_serialization);
    }
}
