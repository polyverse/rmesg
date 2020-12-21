use crate::entry;
use std::convert::From;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::time::SystemTimeError;

#[derive(Debug)]
pub enum RMesgError {
    NotImplementedForThisPlatform,
    UnableToObtainSystemTime,
    UnableToAddDurationToSystemTime,
    KLogTimestampsDisabled,
    IntegerOutOfBound(String),
    Utf8StringConversionError(String),
    IOError(String),
    InternalError(String),
    EntryParsingError(String),
    UnableToObtainElapsedTime(SystemTimeError),
}
impl Error for RMesgError {}
impl Display for RMesgError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "RMesgError:: {}",
            match self {
                Self::NotImplementedForThisPlatform =>
                    "RMesg not implemented for this platform.".to_owned(),
                Self::IntegerOutOfBound(s) => format!("IntegerOutOfBound: {}", s),
                Self::Utf8StringConversionError(s) => format!("Utf8StringConversionError: {}", s),
                Self::IOError(s) => format!("std::io::Error: {}", s),
                Self::InternalError(s) => format!("InternalError: {}", s),
                Self::EntryParsingError(s) => format!("EntryParsingError: {}", s),
                Self::UnableToObtainElapsedTime(s) => format!("UnableToObtainElapsedTime: {}", s),
                Self::UnableToObtainSystemTime => "Failed to get SystemTime.".to_owned(),
                Self::UnableToAddDurationToSystemTime =>
                    "Failed to add a Duration to SystemTime".to_owned(),
                Self::KLogTimestampsDisabled => "Kernel Log timestamps are disabled".to_owned(),
            }
        )
    }
}
impl From<std::string::FromUtf8Error> for RMesgError {
    fn from(err: std::string::FromUtf8Error) -> RMesgError {
        RMesgError::Utf8StringConversionError(format!("{:?}", err))
    }
}

impl From<std::io::Error> for RMesgError {
    fn from(err: std::io::Error) -> RMesgError {
        RMesgError::IOError(format!("{:?}", err))
    }
}

impl From<entry::EntryParsingError> for RMesgError {
    fn from(err: entry::EntryParsingError) -> RMesgError {
        RMesgError::EntryParsingError(format!("{:?}", err))
    }
}
