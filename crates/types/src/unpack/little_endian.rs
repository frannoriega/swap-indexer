use crate::unpack::Unpacker;

pub struct LittleEndianUnpacker<'a> {
    data: &'a [u8],
    position: usize
}

impl<'a> LittleEndianUnpacker<'a> {
    pub fn new(data: &'a [u8]) -> LittleEndianUnpacker<'a> {
        LittleEndianUnpacker {
            data,
            position: 0
        }
    }
}

impl<'a> Unpacker for LittleEndianUnpacker<'a> {
    fn u64(&mut self) -> Result<u64, anyhow::Error> {
        let after_read = self.position + 8;
        // 8 = size of a u64
        if self.data.len() >= after_read {
            let field: u64 = self.data
                .get(self.position..after_read)
                .and_then(|slice| slice.try_into().ok())
                .map(u64::from_le_bytes)
                .ok_or(anyhow::Error::msg("unable to unpack u64"))?;
            self.position = after_read;
            Ok(field)
        } else {
            Err(anyhow::Error::msg("no more data left to unpack"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn unpacks_u64(val: u64) {
            let bytes = val.to_le_bytes();
            let mut unpacker = LittleEndianUnpacker::new(&bytes);
            let unpacked = unpacker.u64().unwrap();

            assert_eq!(val, unpacked);
        }

        #[test]
        fn unpacks_u64_fails_no_more_data(val: u64) {
            let mut unpacker = LittleEndianUnpacker::new(&[0u8, 4u8, 6u8]);
            assert!(unpacker.u64().is_err());
        }
    }
}
