use solana_sdk::pubkey::Pubkey;

#[derive(PartialEq, Clone, Copy, Debug)]
pub struct TransferOperation {
    pub source: Pubkey,
    pub dest: Pubkey,
    pub mint: Option<Pubkey>,
    pub amount: u64,
    pub authority: Pubkey,
}
