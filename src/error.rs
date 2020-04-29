
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug)]
pub enum RMesgError {
    NotImplementedForThisPlatform,
    IntegerOutOfBound(String),
    Utf8StringConversionError(String),
    InternalError(String),
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
                RMesgError::InternalError(s) =>
                    format!("InternalError: {}", s),
            }
        )
    }
}
impl std::convert::From<std::string::FromUtf8Error> for RMesgError {
    fn from(err: std::string::FromUtf8Error) -> RMesgError {
        RMesgError::Utf8StringConversionError(format!("{:?}", err))
    }
}