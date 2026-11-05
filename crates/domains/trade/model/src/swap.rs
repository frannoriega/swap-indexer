use solana_sdk::pubkey::Pubkey;

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct SwapInOperation {
    pub source: Pubkey,
    pub dest: Pubkey,
    pub amount_in: u64,
    pub min_amount_out: u64,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct SwapOutOperation {
    pub source: Pubkey,
    pub dest: Pubkey,
    pub max_amount_in: u64,
    pub amount_out: u64,
}
