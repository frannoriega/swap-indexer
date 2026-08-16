//! Pure swap-mapping logic, decoupled from Carbon and Postgres so it can be
//! unit-tested and benchmarked without a live stream or a database.
//!
//! The processor converts the transaction's token balances into [`TokenBalance`]
//! values and calls [`build_swap_row`]; everything non-trivial (matching pre/post
//! balances, picking the in/out legs, the pair key, the price) lives here.

/// A wallet-relevant SPL token balance, flattened from the transaction meta.
#[derive(Debug, Clone)]
pub struct TokenBalance {
    pub account_index: u8,
    pub mint: String,
    pub owner: String,
    pub amount_raw: u128, // base units
    pub ui_amount: f64,   // decimal-adjusted (0.0 when unknown)
}

/// A swap flattened to the columns of the `swaps` table, independent of the DB
/// layer. The processor turns this into an upsert.
#[derive(Debug, Clone, PartialEq)]
pub struct SwapRow {
    pub signature: String,
    pub wallet: String,
    pub mint_in: String,
    pub mint_out: String,
    pub pair: String,
    pub amount_in: u64,
    pub amount_out: u64,
    pub price: Option<f64>,
    pub slot: i64,
    pub block_time: Option<i64>,
}

struct Leg {
    mint: String,
    raw: u64,
    ui: f64,
}

/// Canonical, order-independent pair key (`min(mint)-max(mint)`). Mints are
/// base58 and contain no '-', so the API can split on '-' safely.
pub fn canonical_pair(a: &str, b: &str) -> String {
    if a <= b {
        format!("{a}-{b}")
    } else {
        format!("{b}-{a}")
    }
}

/// Build a swap row from the wallet's pre/post token balances.
///
/// A swap moves one token out of the wallet (negative delta = input) and another
/// in (positive delta = output). Returns `None` when a wallet-owned in/out pair
/// can't be found — there's nothing honest to store, so the caller skips it.
pub fn build_swap_row(
    signature: String,
    wallet: String,
    slot: u64,
    block_time: Option<i64>,
    pre: &[TokenBalance],
    post: &[TokenBalance],
) -> Option<SwapRow> {
    let (leg_in, leg_out) = resolve_legs(&wallet, pre, post)?;
    let pair = canonical_pair(&leg_in.mint, &leg_out.mint);
    // Naive price: output units per input unit. Null when the input side has no
    // ui amount (missing decimals metadata / zero).
    let price = if leg_in.ui > 0.0 {
        Some(leg_out.ui / leg_in.ui)
    } else {
        None
    };

    Some(SwapRow {
        signature,
        wallet,
        mint_in: leg_in.mint,
        mint_out: leg_out.mint,
        pair,
        amount_in: leg_in.raw,
        amount_out: leg_out.raw,
        price,
        slot: slot as i64,
        block_time,
    })
}

fn resolve_legs(wallet: &str, pre: &[TokenBalance], post: &[TokenBalance]) -> Option<(Leg, Leg)> {
    let mut leg_in: Option<Leg> = None;
    let mut leg_out: Option<Leg> = None;

    for pb in post.iter().filter(|b| b.owner == wallet) {
        let matching_pre = pre.iter().find(|b| b.account_index == pb.account_index);
        let pre_raw = matching_pre.map(|b| b.amount_raw).unwrap_or(0);
        let pre_ui = matching_pre.map(|b| b.ui_amount).unwrap_or(0.0);

        let delta_raw = pb.amount_raw as i128 - pre_raw as i128;
        let delta_ui = pb.ui_amount - pre_ui;

        let leg = Leg {
            mint: pb.mint.clone(),
            raw: delta_raw.unsigned_abs() as u64,
            ui: delta_ui.abs(),
        };

        if delta_raw < 0 {
            leg_in = Some(leg);
        } else if delta_raw > 0 {
            leg_out = Some(leg);
        }
    }

    Some((leg_in?, leg_out?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bal(index: u8, mint: &str, owner: &str, amount_raw: u128, ui_amount: f64) -> TokenBalance {
        TokenBalance {
            account_index: index,
            mint: mint.to_string(),
            owner: owner.to_string(),
            amount_raw,
            ui_amount,
        }
    }

    const WALLET: &str = "WalletAAA";
    const SOL: &str = "So11111111111111111111111111111111111111112";
    const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

    #[test]
    fn maps_input_and_output_legs_by_delta_sign() {
        // Wallet spends 1.5 SOL (index 0) and receives 200 USDC (index 1).
        let pre = vec![
            bal(0, SOL, WALLET, 3_000_000_000, 3.0),
            bal(1, USDC, WALLET, 0, 0.0),
        ];
        let post = vec![
            bal(0, SOL, WALLET, 1_500_000_000, 1.5),
            bal(1, USDC, WALLET, 200_000_000, 200.0),
        ];

        let row = build_swap_row("sig1".into(), WALLET.into(), 42, Some(1_700_000_000), &pre, &post)
            .expect("should resolve a swap");

        assert_eq!(row.mint_in, SOL);
        assert_eq!(row.mint_out, USDC);
        assert_eq!(row.amount_in, 1_500_000_000);
        assert_eq!(row.amount_out, 200_000_000);
        // price = out_ui / in_ui = 200 / 1.5
        assert!((row.price.unwrap() - (200.0 / 1.5)).abs() < 1e-9);
        // pair is canonical regardless of direction
        assert_eq!(row.pair, canonical_pair(SOL, USDC));
    }

    #[test]
    fn ignores_balances_not_owned_by_wallet() {
        // Only a pool-owned balance changes for the wallet's perspective -> no pair.
        let pre = vec![bal(0, SOL, "PoolXXX", 1_000, 0.0)];
        let post = vec![bal(0, SOL, "PoolXXX", 2_000, 0.0)];
        assert!(build_swap_row("sig".into(), WALLET.into(), 1, None, &pre, &post).is_none());
    }

    #[test]
    fn returns_none_without_both_legs() {
        // Wallet only loses a token, never receives one -> incomplete, skip.
        let pre = vec![bal(0, SOL, WALLET, 1_000, 1.0)];
        let post = vec![bal(0, SOL, WALLET, 400, 0.4)];
        assert!(build_swap_row("sig".into(), WALLET.into(), 1, None, &pre, &post).is_none());
    }

    #[test]
    fn canonical_pair_is_order_independent() {
        assert_eq!(canonical_pair(SOL, USDC), canonical_pair(USDC, SOL));
    }
}
