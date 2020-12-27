// Copyright (c) 2019 Polyverse Corporation

use serde::Deserialize;
use serde::Serialize;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::time::Duration;
use strum_macros::EnumString;

#[cfg(feature = "extra-traits")]
use schemars::JsonSchema;

#[cfg(not(feature = "ptr"))]
pub type Entry = EntryStruct;

#[cfg(feature = "ptr")]
pub type Entry = BoxedEntryStruct;
#[cfg(any(feature = "ptr", test))]
pub type BoxedEntryStruct = Box<EntryStruct>;

/// A parsed/structured entry from kernel log buffer
#[derive(PartialEq, Debug, Clone)]
pub struct EntryStruct {
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

impl EntryStruct {
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
    pub fn to_klog_str(&self) -> String {
        let maybe_faclev = self.to_faclev();

        let timestampstr = match self.timestamp_from_system_start {
            Some(ts) => format!("[{: >16.6}]", ts.as_secs_f64()),
            None => "".to_owned(),
        };

        if let Some(faclev) = maybe_faclev {
            format!("<{}>{}{}", faclev, timestampstr, self.message)
        } else {
            self.message.to_string()
        }
    }

    // Like so:
    // 6,1,0,-;Command, line: BOOT_IMAGE=/boot/kernel console=ttyS0 console=ttyS1 page_poison=1 vsyscall=emulate panic=1 root=/dev/sr0 text
    //  LINE2=foobar
    //  LINE 3 = foobar ; with semicolon
    pub fn to_kmsg_str(&self) -> String {
        let maybe_faclev = self.to_faclev();

        let sequence_num = self.sequence_num.unwrap_or(0);

        let timestampstr = match self.timestamp_from_system_start {
            Some(ts) => format!("{}", ts.as_micros()),
            None => "0".to_owned(),
        };

        if let Some(faclev) = maybe_faclev {
            format!(
                "{},{},{},-;{}",
                faclev, sequence_num, timestampstr, self.message
            )
        } else {
            self.message.to_string()
        }
    }
}

impl Display for EntryStruct {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let timestampstr = match self.timestamp_from_system_start {
            Some(ts) => format!("[{: >16.6}]", ts.as_secs_f64()),
            None => "".to_owned(),
        };

        write!(f, "{}{}", timestampstr, self.message)
    }
}

/// Linux kmesg (kernel message buffer) Log Facility.
#[cfg_attr(feature = "extra-traits", derive(JsonSchema, Deserialize))]
#[derive(EnumString, Debug, PartialEq, Display, Copy, Clone, FromPrimitive, Serialize)]
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
#[cfg_attr(feature = "extra-traits", derive(JsonSchema, Deserialize))]
#[derive(
    EnumString,
    Debug,
    PartialEq,
    Display,
    Copy,
    Clone,
    FromPrimitive,
    Serialize,
)]
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
mod test {
    use super::*;
    use std::mem;

    #[test]
    fn test_entry_5x_larger_than_box() {
        // cost to move Entry
        assert_eq!(72, mem::size_of::<EntryStruct>());
        // vs ptr
        assert_eq!(8, mem::size_of::<BoxedEntryStruct>());

        assert!(mem::size_of::<EntryStruct>() >= mem::size_of::<BoxedEntryStruct>() * 5);
    }

    #[test]
    fn test_serialize_to_klog() {
        let entry_struct = EntryStruct {
            timestamp_from_system_start: Some(Duration::from_secs_f64(24241.325252)),
            facility: Some(LogFacility::Kern),
            level: Some(LogLevel::Info),
            sequence_num: Some(10),
            message: "Test message".to_owned(),
        };
        let expected_serialization = "<6>[    24241.325252]Test message";

        let boxed_entry_struct = Box::new(entry_struct.clone());

        let printed_entry_struct = entry_struct.to_klog_str();
        assert_eq!(printed_entry_struct, expected_serialization);

        let printed_boxed_entry_struct = boxed_entry_struct.to_klog_str();
        assert_eq!(printed_boxed_entry_struct, expected_serialization);
    }

    #[test]
    fn test_serialize_to_kmsg() {
        let entry_struct = EntryStruct {
            timestamp_from_system_start: Some(Duration::from_secs_f64(24241.325252)),
            facility: Some(LogFacility::Kern),
            level: Some(LogLevel::Info),
            sequence_num: Some(23),
            message: "Test message".to_owned(),
        };
        let expected_serialization = "6,23,24241325252,-;Test message";

        let boxed_entry_struct = Box::new(entry_struct.clone());

        let printed_entry_struct = entry_struct.to_kmsg_str();
        assert_eq!(printed_entry_struct, expected_serialization);

        let printed_boxed_entry_struct = boxed_entry_struct.to_kmsg_str();
        assert_eq!(printed_boxed_entry_struct, expected_serialization);
    }

    #[test]
    fn test_display() {
        let entry_struct = EntryStruct {
            timestamp_from_system_start: Some(Duration::from_secs_f64(24241.325252)),
            facility: Some(LogFacility::Kern),
            level: Some(LogLevel::Info),
            sequence_num: Some(15),
            message: "Test message".to_owned(),
        };
        let expected_serialization = "[    24241.325252]Test message";

        let boxed_entry_struct = Box::new(entry_struct.clone());

        let printed_entry_struct = format!("{}", entry_struct);
        assert_eq!(printed_entry_struct, expected_serialization);

        let printed_boxed_entry_struct = format!("{}", boxed_entry_struct);
        assert_eq!(printed_boxed_entry_struct, expected_serialization);
    }
}
