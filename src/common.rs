use crate::entry::{EntryParsingError, LogFacility, LogLevel};
use num::FromPrimitive;
use std::any::type_name;
use std::fmt::Display;
use std::str::FromStr;
use std::time::Duration;

const LEVEL_MASK: u32 = (1 << 3) - 1;

pub fn parse_favlecstr(
    faclevstr: &str,
    line: &str,
) -> Result<(LogFacility, LogLevel), EntryParsingError> {
    match parse_fragment::<u32>(faclevstr) {
        Some(faclev) => {
            // facility is top 28 bits, log level is bottom 3 bits
            match (
                LogFacility::from_u32(faclev >> 3),
                LogLevel::from_u32(faclev & LEVEL_MASK),
            ) {
                (Some(facility), Some(level)) => Ok((facility, level)),
                _ => Err(EntryParsingError::Generic(format!(
                    "Unable to parse {} into log facility and level. Line: {}",
                    faclev, line
                ))),
            }
        }
        None => Err(EntryParsingError::Generic(format!(
            "Unable to parse facility/level {} into a base-10 32-bit unsigned integer. Line: {}",
            faclevstr, line
        ))),
    }
}

pub fn parse_timestamp_secs(
    timestampstr: &str,
    line: &str,
) -> Result<Option<Duration>, EntryParsingError> {
    match parse_fragment::<f64>(timestampstr) {
        Some(timesecs) => Ok(Some(Duration::from_secs_f64(timesecs))),
        None => Err(EntryParsingError::Generic(format!(
            "Unable to parse {} into a floating point number. Line: {}",
            timestampstr, line,
        ))),
    }
}

pub fn parse_fragment<N: FromStr>(frag: &str) -> Option<N>
where
    N::Err: Display,
{
    match frag.trim().parse() {
        Ok(f) => Some(f),
        Err(e) => {
            eprintln!("Unable to parse {} into {}: {}", frag, type_name::<N>(), e);
            None
        }
    }
}
