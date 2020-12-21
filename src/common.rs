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
) -> Result<(Option<LogFacility>, Option<LogLevel>), EntryParsingError> {
    let faclev = parse_fragment::<u32>(faclevstr, line)?;
    // facility is top 28 bits, log level is bottom 3 bits
    match (
        LogFacility::from_u32(faclev >> 3),
        LogLevel::from_u32(faclev & LEVEL_MASK),
    ) {
        (Some(facility), Some(level)) => Ok((Some(facility), Some(level))),
        _ => Err(EntryParsingError::Generic(format!(
            "Unable to parse {} into log facility and level. Line: {}",
            faclev, line
        ))),
    }
}

pub fn parse_timestamp_secs(
    timestampstr: &str,
    line: &str,
) -> Result<Option<Duration>, EntryParsingError> {
    Ok(Some(Duration::from_secs_f64(parse_fragment::<f64>(
        timestampstr,
        line,
    )?)))
}

pub fn parse_timestamp_microsecs(
    timestampstr: &str,
    line: &str,
) -> Result<Option<Duration>, EntryParsingError> {
    Ok(Some(Duration::from_micros(parse_fragment::<u64>(
        timestampstr,
        line,
    )?)))
}

pub fn parse_fragment<N: FromStr>(frag: &str, line: &str) -> Result<N, EntryParsingError>
where
    N::Err: Display,
{
    match frag.trim().parse() {
        Ok(f) => Ok(f),
        Err(e) => Err(EntryParsingError::Generic(format!(
            "Unable to parse {} into a {} due to error: {}\nLine: {}",
            frag,
            type_name::<N>(),
            e,
            line,
        ))),
    }
}
