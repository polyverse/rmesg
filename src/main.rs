#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
use std::convert::TryFrom;

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug)]
pub enum RMesgError {
    NotImplementedForThisPlatform,
    IntegerOutOfBound(String),
    KLogCtlCallError(String),
    KLogCtlInternalError(String),
    Utf8StringConversionError(String),
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
                RMesgError::KLogCtlCallError(s) =>
                    format!("An error occurred in how KLogCtl was called: {}", s),
                RMesgError::KLogCtlInternalError(s) =>
                    format!("An error was returned by KLogCtl: {}", s),
                RMesgError::IntegerOutOfBound(s) => format!("IntegerOutOfBound: {}", s),
                RMesgError::Utf8StringConversionError(s) =>
                    format!("Utf8StringConversionError: {}", s),
            }
        )
    }
}
impl std::convert::From<std::string::FromUtf8Error> for RMesgError {
    fn from(err: std::string::FromUtf8Error) -> RMesgError {
        RMesgError::Utf8StringConversionError(format!("{:?}", err))
    }
}

#[cfg(target_os = "linux")]
impl std::convert::From<linux::KLogCtlError> for RMesgError {
    fn from(e: linux::KLogCtlError) -> RMesgError {
        RMesgError::KLogCtlCallError(format!("{:?}", e))
    }
}

// acts as a library when needed
#[cfg(target_os = "linux")]
fn rmesg() -> Result<String, RMesgError> {
    let mut dummy_buffer: [i8; 0] = [];
    let buf_size = linux::safe_klogctl(
        linux::KLogType::SyslogActionSizeBuffer,
        dummy_buffer.as_mut_ptr(),
        dummy_buffer.len(),
    )?;
    let buf_usize = match usize::try_from(buf_size) {
        Ok(u) => u,
        Err(e) => {
            return Err(RMesgError::IntegerOutOfBound(format!(
                "Error converting {} into a usize: {:?}",
                buf_size, e
            )))
        }
    };

    let mut real_buffer: Vec<u8> = vec![0; buf_usize];
    let len_i32 = linux::safe_klogctl(
        linux::KLogType::SyslogActionReadAll,
        real_buffer.as_mut_ptr() as *mut i8,
        real_buffer.capacity(),
    )?;

    if len_i32 < 0 {
        return Err(RMesgError::KLogCtlInternalError(format!(
            "Error from klogctl. Unable to read buffer. Code: {}",
            len_i32
        )));
    }
    let len_usize = match usize::try_from(len_i32) {
        Ok(u) => u,
        Err(e) => {
            return Err(RMesgError::IntegerOutOfBound(format!(
                "Error converting {} into a usize: {:?}",
                len_i32, e
            )))
        }
    };

    real_buffer.resize(len_usize, 0);

    //adjust buffer capacity to what was read
    real_buffer.resize(len_usize, 0);
    Ok(String::from_utf8(real_buffer)?)
}

#[cfg(not(target_os = "linux"))]
fn rmesg() -> Result<String, RMesgError> {
    Err(RMesgError::NotImplementedForThisPlatform)
}

fn main() {
    println!("{}", rmesg().unwrap())
}

/**********************************************************************************/
// Tests! Tests! Tests!

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn get_kernel_buffer_size() {
        let kmsg = rmesg().unwrap();
        println!("Kernel message buffer contents:\n{}", kmsg);
    }
}
