pub mod little_endian;
pub mod big_endian;

pub trait Unpacker {
    fn u64(&mut self) -> Result<u64, anyhow::Error>;
}
