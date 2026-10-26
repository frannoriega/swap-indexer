use std::ops::{Range, RangeInclusive};

use super::*;
use ::proptest::prelude::*;

pub enum Length {
    Exact(usize),
    Range(RangeInclusive<usize>)
}

impl From<usize> for Length {
    fn from(value: usize) -> Self {
        Length::Exact(value)
    }
}

impl From<Range<usize>> for Length {
    fn from(value: Range<usize>) -> Self {
        Length::Range(value.start..=(value.end - 1))
    }
}

impl From<RangeInclusive<usize>> for Length {
    fn from(value: RangeInclusive<usize>) -> Self {
        Length::Range(value)
    }
}

pub fn addresses(len_range: impl Into<Length>) -> impl Strategy<Value = Vec<Pubkey>> {
    let range: Range<usize> = match len_range.into() {
        Length::Exact(val) => val..(val + 1),
        Length::Range(range) => *range.start()..(*range.end() + 1),
    };
    prop::collection::vec(
        any::<[u8; 32]>().prop_map(Pubkey::new_from_array),
        range,
    )
}

pub fn address() -> impl Strategy<Value = Pubkey> {
    any::<[u8;32]>().prop_map(Pubkey::new_from_array)
}
