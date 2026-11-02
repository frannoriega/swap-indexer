use std::collections::HashMap;

use solana_sdk::pubkey::Pubkey;
use types::RawTx;

/// Information about a token account (its mint address + the decimals the token uses)
#[derive(Copy, Clone)]
pub struct AccountInfo {
    pub mint: Pubkey,
    pub decimals: u8,
}

/// Human-facing identity for a mint (symbol/name).
#[derive(Clone)]
pub struct TokenInfo {
    pub symbol: String,
    pub name: Option<String>,
}

/// Unlike [AccountInfo] (mint/decimals, free from the tx itself), the
/// [TokenInfo] doesn't exist anywhere in the transaction: Solana instructions
/// never carry a token's symbol, so getting it requires either a static
/// table of well-known mints or a live lookup (Metaplex metadata, a hosted
/// token list, etc).
/// That's why this is a trait, not a field.
pub trait TokenResolver {
    fn resolve(&self, mint: &Pubkey) -> Option<TokenInfo>;
}

/// The simplest possible [TokenResolver]: a fixed table of known mints,
/// no I/O. A good default for the common tokens (SOL, USDC, ...), and a
/// starting point for anyone who wants to swap in a live resolver instead.
pub struct StaticTokenResolver {
    known: HashMap<Pubkey, TokenInfo>,
}

impl StaticTokenResolver {
    pub fn new(known: HashMap<Pubkey, TokenInfo>) -> Self {
        StaticTokenResolver { known }
    }
}

impl TokenResolver for StaticTokenResolver {
    fn resolve(&self, mint: &Pubkey) -> Option<TokenInfo> {
        self.known.get(mint).cloned()
    }
}

/// Traverses the token balances and maps the token accounts to its information
pub fn token_accounts(tx: &RawTx) -> HashMap<Pubkey, AccountInfo> {
    let mut accounts = HashMap::new();
    for balance in tx
        .pre_token_balances
        .iter()
        .chain(tx.post_token_balances.iter())
    {
        accounts.insert(
            balance.account,
            AccountInfo {
                mint: balance.mint,
                decimals: balance.decimals,
            },
        );
    }
    accounts
}
