// Copyright (c) 2019 Polyverse Corporation

use serde::Serialize;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::time::Duration;
use strum_macros::EnumString;

#[cfg(test)]
use schemars::JsonSchema;
#[cfg(test)]
use serde::Deserialize;

#[cfg(not(feature = "ptr"))]
pub type Entry = EntryStruct;

#[cfg(feature = "ptr")]
pub type Entry = BoxedEntryStruct;
#[cfg(any(feature = "ptr", test))]
pub type BoxedEntryStruct = Box<EntryStruct>;

/// A parsed/structured entry from kernel log buffer
#[derive(PartialEq, Debug, Clone)]
#[cfg_attr(test, derive(JsonSchema, Deserialize))]
pub struct EntryStruct {
    // The amount of time since system bootstrapped
    pub timestamp_from_system_start: Option<Duration>,

    // Log facility
    pub facility: LogFacility,

    // Log level
    pub level: LogLevel,

    // Log message
    pub message: String,
}

impl Display for EntryStruct {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let faclev = ((self.facility as u8) << 3) + (self.level as u8);
        let timestampstr = match self.timestamp_from_system_start {
            Some(ts) => format!("[{: >16}]", ts.as_secs_f64()),
            None => "".to_owned(),
        };

        write!(f, "<{}>{}{}", faclev, timestampstr, self.message)
    }
}

/// Linux kmesg (kernel message buffer) Log Facility.
#[derive(EnumString, Debug, PartialEq, Display, Copy, Clone, FromPrimitive, Serialize)]
#[cfg_attr(test, derive(JsonSchema, Deserialize))]
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
#[derive(EnumString, Debug, PartialEq, Display, Copy, Clone, FromPrimitive, Serialize)]
#[cfg_attr(test, derive(JsonSchema, Deserialize))]
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
        assert_eq!(56, mem::size_of::<EntryStruct>());
        // vs ptr
        assert_eq!(8, mem::size_of::<BoxedEntryStruct>());

        assert!(mem::size_of::<EntryStruct>() >= mem::size_of::<BoxedEntryStruct>() * 5);
    }

    #[test]
    fn test_serialize() {
        let entry_struct = EntryStruct {
            timestamp_from_system_start: Some(Duration::from_secs_f64(24241.325252)),
            facility: LogFacility::Kern,
            level: LogLevel::Info,
            message: "Test message".to_owned(),
        };
        let expected_serialization = "<6>[    24241.325252]Test message";

        let boxed_entry_struct = Box::new(entry_struct.clone());

        let printed_entry_struct = format!("{}", entry_struct);
        assert_eq!(printed_entry_struct, expected_serialization);

        let printed_boxed_entry_struct = format!("{}", boxed_entry_struct);
        assert_eq!(printed_boxed_entry_struct, expected_serialization);
    }
}
