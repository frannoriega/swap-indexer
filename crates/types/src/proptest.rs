use std::ops::{Range, RangeInclusive};

use super::*;
use ::proptest::prelude::*;

pub enum Length {
    Exact(u8),
    Range(RangeInclusive<u8>)
}

impl From<u8> for Length {
    fn from(value: u8) -> Self {
        Length::Exact(value)
    }
}

impl From<Range<u8>> for Length {
    fn from(value: Range<u8>) -> Self {
        Length::Range(value.start..=(value.end - 1))
    }
}

impl From<RangeInclusive<u8>> for Length {
    fn from(value: RangeInclusive<u8>) -> Self {
        Length::Range(value)
    }
}

pub fn addresses(len_range: impl Into<Length>) -> impl Strategy<Value = Vec<Pubkey>> {
    let range: Range<usize> = match len_range.into() {
        Length::Exact(val) => val as usize..(val as usize + 1),
        Length::Range(range) => *range.start() as usize..(*range.end() as usize + 1),
    };
    prop::collection::vec(
        any::<[u8; 32]>().prop_map(Pubkey::new_from_array),
        range,
    )
}
