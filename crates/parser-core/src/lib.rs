use std::marker::PhantomData;

use types::{ParsedInstruction, ParsedTx, InstructionParser, RawTx};

/**
 * Transaction parser that uses a specific inner [Parser]
 * to retrieve instructions
 */
pub struct TxParser<P, T>
where P: InstructionParser<T> {
    instruction_parser: P,
    // Used to pin the `T` type, the output of the parser
    _output: PhantomData<T>
}

impl<P, T> TxParser<P, T>
where P: InstructionParser<T> {
    pub fn new(parser: P) -> TxParser<P, T> {
        TxParser {
            instruction_parser: parser,
            _output: PhantomData
        }
    }

    pub fn parse_transaction(&self, tx: RawTx) -> Result<ParsedTx<T>, anyhow::Error> {
        let mut instructions: Vec<ParsedInstruction<T>> = Vec::new();
        for instr in tx.instructions {
            let parsed = self.instruction_parser.parse(instr.data.as_slice())?;
            instructions.push(ParsedInstruction {
                program_id: instr.program_id,
                accounts: instr.accounts.clone(),
                data: parsed
            });
        }
        Ok(ParsedTx { 
            signature: tx.signature,
            slot: tx.slot,
            block_time: tx.block_time,
            account_keys: tx.account_keys,
            instructions,
            status: tx.status
        })
    }
}
