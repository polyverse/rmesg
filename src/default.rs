use crate::error::RMesgError;

// Default Implementation of platform-specfic methods.
pub fn rmesg() -> Result<String, RMesgError> {
    Err(RMesgError::NotImplementedForThisPlatform)
}
