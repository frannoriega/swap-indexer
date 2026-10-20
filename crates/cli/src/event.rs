pub struct Swap {
    pub source_mint: Pubkey,
    pub dest_mint: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub signer: Pubkey
}
