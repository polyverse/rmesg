use libc;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

// Can be removed once upstream libc supports it.
extern "C" {
    fn klogctl(syslog_type: libc::c_int, buf: *mut libc::c_char, len: libc::c_int) -> libc::c_int;
}

#[derive(Debug)]
pub enum KLogCtlError {
    IntegerOutOfBound(String),
}
impl Error for KLogCtlError {}
impl Display for KLogCtlError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "KLogCtlError:: {}",
            match self {
                KLogCtlError::IntegerOutOfBound(s) => format!("{}", s),
            }
        )
    }
}

// SYSLOG constants
// https://linux.die.net/man/3/klogctl
pub enum KLogType {
    SyslogActionClose,
    SyslogActionOpen,
    SyslogActionRead,
    SyslogActionReadAll,
    SyslogActionReadClear,
    SyslogActionClear,
    SyslogActionConsoleOff,
    SyslogActionConsoleOn,
    SyslogActionConsoleLevel,
    SyslogActionSizeUnread,
    SyslogActionSizeBuffer,
}

type SignedInt = i32;

// klogctl implementation from MUSL
// https://github.com/rofl0r/musl/blob/master/src/linux/klogctl.c
pub fn safe_klogctl(
    klogtype: KLogType,
    buf: *mut i8,
    buflen_usize: usize,
) -> Result<SignedInt, KLogCtlError> {
    let type_signed_int = klogtype as SignedInt;
    let klt: libc::c_int = type_signed_int;
    let buflen: i32 = match i32::try_from(buflen_usize) {
        Ok(i) => i,
        Err(e) => {
            return Err(KLogCtlError::IntegerOutOfBound(format!(
                "Error converting usize {} into i32: {:?}",
                buflen_usize, e
            )))
        }
    };
    unsafe {
        let response: libc::c_int = klogctl(klt, buf, buflen);
        *buf = 10;
        let rusty_response: SignedInt = response;
        return Ok(rusty_response);
    }
}

/**********************************************************************************/
// Tests! Tests! Tests!

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn get_kernel_buffer_size() {
        let mut buf: [i8; 0] = [];
        let response = safe_klogctl(
            KLogType::SyslogActionSizeBuffer,
            buf.as_mut_ptr(),
            buf.len(),
        );
        println!("Kernel message buffer size: {}", response.unwrap());
    }
}
