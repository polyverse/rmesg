use crate::error::RMesgError;

// Default Implementation of platform-specfic methods.
pub fn rmesg(_clear: bool) -> Result<String, RMesgError> {
    Err(RMesgError::NotImplementedForThisPlatform)
}
