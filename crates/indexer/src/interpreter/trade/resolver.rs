use std::collections::HashMap;

use solana_sdk::pubkey::Pubkey;

use crate::accounts::TokenResolver;
use crate::interpreter::trade::event::{Legs, TokenAmount, TradeEvent};

/// A [TokenAmount] once resolution has been attempted.
/// `symbol` is `None` when the resolver has nothing for this mint (or the mint itself wasn't
/// known).
/// This is a best-effort enrichment, unlike the amount/decimals, which are always known.
pub struct ResolvedTokenAmount {
    pub amount: u64,
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
}

/// One side of a [ResolvedTradeEvent], mirroring [Legs]: keyed by account
/// so a multi-resource trade (see [Legs]) resolves to one entry per
/// resource, same as the unresolved version.
pub type ResolvedLegs = HashMap<Pubkey, ResolvedTokenAmount>;

pub struct ResolvedTradeEvent {
    pub program_id: Pubkey,
    pub input: ResolvedLegs,
    pub output: ResolvedLegs,
}

/// Explicit, optional enrichment pass over an already-built [TradeEvent] -
/// deliberately not part of [crate::interpreter::Interpreter] or
/// [crate::interpreter::Event] itself, since not every consumer wants
/// (or can pay for) resolving token identity.
pub fn resolve(event: &TradeEvent, resolver: &dyn TokenResolver) -> ResolvedTradeEvent {
    let resolve_leg = |leg: &TokenAmount| ResolvedTokenAmount {
        amount: leg.amount,
        decimals: leg.token.map(|t| t.decimals),
        symbol: leg
            .token
            .map(|t| t.mint)
            .and_then(|mint| resolver.resolve(&mint))
            .map(|info| info.symbol),
    };
    let resolve_side = |legs: &Legs| -> ResolvedLegs {
        legs.iter()
            .map(|(account, leg)| (*account, resolve_leg(leg)))
            .collect()
    };

    ResolvedTradeEvent {
        program_id: event.program_id,
        input: resolve_side(&event.input),
        output: resolve_side(&event.output),
    }
}
