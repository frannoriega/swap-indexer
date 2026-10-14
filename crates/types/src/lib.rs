pub mod unpack;

pub trait Parser<T> {
    fn parse(instruction_data: &[u8]) -> Result<T, anyhow::Error>;
}

