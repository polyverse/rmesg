use crate::error::RMesgError;

use errno::errno;
use libc;
use std::convert::TryFrom;
use std::fmt::Display;

// Can be removed once upstream libc supports it.
extern "C" {
    fn klogctl(syslog_type: libc::c_int, buf: *mut libc::c_char, len: libc::c_int) -> libc::c_int;
}

// SYSLOG constants
// https://linux.die.net/man/3/klogctl
#[derive(Debug, Display, Clone)]
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

pub type SignedInt = libc::c_int;

/*
    Safely wraps the klogctl for Rusty types
*/
pub fn safely_wrapped_klogctl(klogtype: KLogType, buf_u8: &mut [u8]) -> Result<usize, RMesgError> {
    // convert klogtype
    let klt = klogtype.clone() as libc::c_int;

    // extract mutable u8 raw pointer from buf
    // and typecast it (very dangerously) to i8
    // fortunately it's all one-byte long so
    // should be reasonably okay.
    let buf_i8 = buf_u8.as_mut_ptr() as *mut i8;

    let buflen = match libc::c_int::try_from(buf_u8.len()) {
        Ok(i) => i,
        Err(e) => {
            return Err(RMesgError::IntegerOutOfBound(format!(
                "Error converting buffer length for klogctl from <usize>::({}) into <c_int>: {:?}",
                buf_u8.len(),
                e
            )))
        }
    };

    let response_cint: libc::c_int = unsafe { klogctl(klt, buf_i8, buflen) };

    if response_cint < 0 {
        let err = errno();
        return Err(RMesgError::InternalError(format!(
            "Request ({}) to klogctl failed. errno={}",
            klogtype, err
        )));
    }

    let response = match usize::try_from(response_cint) {
        Ok(i) => i,
        Err(e) => {
            return Err(RMesgError::IntegerOutOfBound(format!(
                "Error converting response from klogctl from <c_int>::({}) into <usize>: {:?}",
                response_cint, e
            )))
        }
    };

    return Ok(response);
}

pub fn rmesg(clear: bool) -> Result<String, RMesgError> {
    let mut dummy_buffer: Vec<u8> = vec![0; 0];
    let kernel_buffer_size =
        safely_wrapped_klogctl(KLogType::SyslogActionSizeBuffer, &mut dummy_buffer)?;

    let klogtype = match clear {
        true => KLogType::SyslogActionReadClear,
        false => KLogType::SyslogActionReadAll,
    };

    let mut real_buffer: Vec<u8> = vec![0; kernel_buffer_size];
    let bytes_read = safely_wrapped_klogctl(klogtype, &mut real_buffer)?;

    //adjust buffer capacity to what was read
    real_buffer.resize(bytes_read, 0);
    let utf8_str = String::from_utf8(real_buffer)?;

    // if incremental,
    Ok(utf8_str)
}

/**********************************************************************************/
// Tests! Tests! Tests!

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn get_kernel_buffer_size() {
        let mut dummy_buffer: Vec<u8> = vec![0; 0];
        let response = safely_wrapped_klogctl(KLogType::SyslogActionSizeBuffer, &mut dummy_buffer);
        assert!(response.is_ok(), "Failed to call klogctl");
        assert!(
            response.unwrap() > 0,
            "Buffer size should be greater than zero."
        );
    }

    #[test]
    fn test_rmesg() {
        let logs = rmesg(false);
        assert!(logs.is_ok(), "Failed to call rmesg");
        assert!(logs.unwrap().len() > 0, "Should have non-empty logs");
    }
}
