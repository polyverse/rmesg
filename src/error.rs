use std::convert::From;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug)]
pub enum RMesgError {
    NotImplementedForThisPlatform,
    IntegerOutOfBound(String),
    Utf8StringConversionError(String),
    IOError(String),
    InternalError(String),
    UnableToObtainSystemTime,
    UnableToAddDurationToSystemTime,
}
impl Error for RMesgError {}
impl Display for RMesgError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "RMesgError:: {}",
            match self {
                RMesgError::NotImplementedForThisPlatform =>
                    "RMesg not implemented for this platform.".to_owned(),
                RMesgError::IntegerOutOfBound(s) => format!("IntegerOutOfBound: {}", s),
                RMesgError::Utf8StringConversionError(s) =>
                    format!("Utf8StringConversionError: {}", s),
                RMesgError::IOError(s) => format!("std::io::Error: {}", s),
                RMesgError::InternalError(s) => format!("InternalError: {}", s),
                RMesgError::UnableToObtainSystemTime => "Failed to get SystemTime.".to_owned(),
                RMesgError::UnableToAddDurationToSystemTime =>
                    "Failed to add a Duration to SystemTime".to_owned(),
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
