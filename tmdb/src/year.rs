use core::fmt;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize};
use std::num::NonZeroU16;

#[derive(fmt::Debug, Clone, Copy, Serialize)]
/// Years can only be in the range 1000..9999
pub struct Year(NonZeroU16);

impl Year {
    #[must_use]
    #[inline]
    pub const fn new(year: u16) -> Option<Self> {
        if year > 1000 && year <= 9999 {
            // SAFETY: year is greater than 1000 which is always greater than 0
            unsafe { Some(Year(NonZeroU16::new_unchecked(year))) }
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn from_nonzero_u16(value: NonZeroU16) -> Option<Self> {
        if value.get() > 1000 && value.get() <= 9999 {
            Some(Year(value))
        } else {
            None
        }
    }
}

impl fmt::Display for Year {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for Year {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let non_zero = NonZeroU16::deserialize(deserializer)?;
        match Year::from_nonzero_u16(non_zero) {
            Some(y) => Ok(y),
            None => Err(D::Error::custom(format_args!(
                "invalid year: {non_zero}, valid range is 1000..=9999"
            ))),
        }
    }
}
