/**
 * Exact input swap instruction.
 * It models the exact amount to spend and the least amount to receive.
 */
#[derive(Debug, PartialEq)]
pub struct SwapBaseInInstruction {
    /**
     * The amount of source tokens a wallet is swapping for the destination token.
     */
    pub amount_in: u64,
    /**
     * The minimum amount of destination tokens (meaning, after swap) to be outputted.
     * This is meant as a safety guard in case of a huge slippage.
     */
    pub minimum_amount_out: u64
}

/**
 * Exact output swap instruction.
 * It models the exact amount to receive and the maximum amount to spend.
 */
#[derive(Debug, PartialEq)]
pub struct SwapBaseOutInstruction {
    /**
     * The maximum amount of source tokens to be swapped.
     * This is a safety against a huge slippage.
     */
    pub max_amount_in: u64,
    /**
     * The amount of destination tokens requested.
     */
    pub amount_out: u64
}

#[derive(Debug, PartialEq)]
pub enum AmmInstruction {
    SwapBaseIn(SwapBaseInInstruction),
    SwapBaseOut(SwapBaseOutInstruction),
    SwapBaseInV2(SwapBaseInInstruction),
    SwapBaseOutV2(SwapBaseOutInstruction)
}
