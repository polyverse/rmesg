
#[cfg(target_os="linux")]
pub mod linux;

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::convert::TryFrom;

#[derive(Debug)]
pub enum RMesgError {
    NotImplementedForThisPlatform,
    IntegerOutOfBound(String),
    KLogCtlCallError(String),
    KLogCtlInternalError(String),
}
impl Error for RMesgError{}
impl Display for RMesgError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "RMesgError:: {}",
            match self {
                RMesgError::NotImplementedForThisPlatform => "RMesg not implemented for this platform.".to_owned(),
                RMesgError::KLogCtlCallError(s) => format!("An error occurred in how KLogCtl was called: {}", s),
                RMesgError::KLogCtlInternalError(s) => format!("An error was returned by KLogCtl: {}", s),
                RMesgError::IntegerOutOfBound(s) => format!("IntegerOutOfBound: {}", s),
            }
        )
    }
}

#[cfg(target_os="linux")]
impl std::convert::From<linux::KLogCtlError> for RMesgError {
    fn from(e: linux::KLogCtlError) -> RMesgError {
        RMesgError::KLogCtlCallError(format!("{:?}", e))
    }
}

// acts as a library when needed
fn rmesg() -> Result<String, RMesgError> {
    if cfg!(target_os="linux") {
        let mut dummy_buffer = String::from("");
        let buf_size = linux::safe_klogctl(linux::KLogType::SyslogActionSizeBuffer, &mut dummy_buffer)?;
        let buf_usize = match usize::try_from(buf_size) {
            Ok(u) => u,
            Err(e) => return Err(RMesgError::IntegerOutOfBound(format!("Error converting {} into a usize: {:?}", buf_size, e))),
        };
        let mut real_buffer = String::with_capacity(buf_usize);
        let len = linux::safe_klogctl(linux::KLogType::SyslogActionReadAll, &mut real_buffer)?;
        if len < 0 {
            return Err(RMesgError::KLogCtlInternalError(format!("Error from klogctl. Unable to read buffer. Code: {}", len)))
        }
        //adjust buffer capacity to what was read
        println!("Number of bytes read: {}", len);
        Ok(real_buffer)
    } else {
        Err(RMesgError::NotImplementedForThisPlatform)
    }
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
