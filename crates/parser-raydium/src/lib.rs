pub mod instruction;
use types::{Parser, unpack::{Unpacker, little_endian::LittleEndianUnpacker}};

use crate::instruction::AmmInstruction;

pub struct RaydiumParser;

impl Parser<AmmInstruction> for RaydiumParser {
    fn parse(instruction_data: &[u8]) -> Result<AmmInstruction, anyhow::Error> {
        let (id, rest) = instruction_data.split_first()
            .ok_or(anyhow::Error::msg("invalid instruction"))?;
        let mut unpacker = LittleEndianUnpacker::new(rest);
        Ok(match id {
            // SwapBaseIn
            9 => {
                let amount_in = unpacker.u64()?;
                let minimum_amount_out = unpacker.u64()?;
                AmmInstruction::SwapBaseIn(instruction::SwapBaseInInstruction { amount_in, minimum_amount_out })
            },
            // SwapBaseOut
            11 => {
                let max_amount_in = unpacker.u64()?;
                let amount_out = unpacker.u64()?;
                AmmInstruction::SwapBaseOut(instruction::SwapBaseOutInstruction { max_amount_in, amount_out })
            }
            // SwapBaseInV2
            16 => {
                let amount_in = unpacker.u64()?;
                let minimum_amount_out = unpacker.u64()?;
                AmmInstruction::SwapBaseInV2(instruction::SwapBaseInInstruction { amount_in, minimum_amount_out })
            }
            // SwapBaseOutV2
            17 => {
                let max_amount_in = unpacker.u64()?;
                let amount_out = unpacker.u64()?;
                AmmInstruction::SwapBaseOutV2(instruction::SwapBaseOutInstruction { max_amount_in, amount_out })
            }
            _ => return Err(anyhow::Error::msg("instruction variant not supported"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {

        #[test]
        fn parses_swap_base_in(amount_in: u64, minimum_amount_out: u64) {
            let mut instruction = Vec::<u8>::new();
            instruction.push(9u8);
            instruction.extend_from_slice(&amount_in.to_le_bytes());
            instruction.extend_from_slice(&minimum_amount_out.to_le_bytes());

            let parsed = RaydiumParser::parse(instruction.as_slice()).unwrap();
            assert_eq!(
                AmmInstruction::SwapBaseIn(instruction::SwapBaseInInstruction { amount_in, minimum_amount_out }),
                parsed
            )
        }

        #[test]
        fn parses_swap_base_out(max_amount_in: u64, amount_out: u64) {
            let mut instruction = Vec::<u8>::new();
            instruction.push(11u8);
            instruction.extend_from_slice(&max_amount_in.to_le_bytes());
            instruction.extend_from_slice(&amount_out.to_le_bytes());

            let parsed = RaydiumParser::parse(instruction.as_slice()).unwrap();
            assert_eq!(
                AmmInstruction::SwapBaseOut(instruction::SwapBaseOutInstruction { max_amount_in, amount_out }),
                parsed
            )
        }

        #[test]
        fn parses_swap_base_in_v2(amount_in: u64, minimum_amount_out: u64) {
            let mut instruction = Vec::<u8>::new();
            instruction.push(16u8);
            instruction.extend_from_slice(&amount_in.to_le_bytes());
            instruction.extend_from_slice(&minimum_amount_out.to_le_bytes());

            let parsed = RaydiumParser::parse(instruction.as_slice()).unwrap();
            assert_eq!(
                AmmInstruction::SwapBaseInV2(instruction::SwapBaseInInstruction { amount_in, minimum_amount_out }),
                parsed
            )
        }

        #[test]
        fn parses_swap_base_out_v2(max_amount_in: u64, amount_out: u64) {
            let mut instruction = Vec::<u8>::new();
            instruction.push(17u8);
            instruction.extend_from_slice(&max_amount_in.to_le_bytes());
            instruction.extend_from_slice(&amount_out.to_le_bytes());

            let parsed = RaydiumParser::parse(instruction.as_slice()).unwrap();
            assert_eq!(
                AmmInstruction::SwapBaseOutV2(instruction::SwapBaseOutInstruction { max_amount_in, amount_out }),
                parsed
            )
        }
    }

}
