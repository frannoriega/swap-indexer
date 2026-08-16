//! Micro-benchmark: how fast can we turn a decoded swap's token balances into a
//! `SwapRow`? This is the CPU-bound cost per swap in the pipeline (everything
//! except the network stream and the DB write).
//!
//! Run: `cargo bench -p ingestion`

use criterion::{criterion_group, criterion_main, Criterion};
use ingestion::mapping::{build_swap_row, TokenBalance};
use std::hint::black_box;

fn sample_balances() -> (Vec<TokenBalance>, Vec<TokenBalance>) {
    let wallet = "So1swapWa11etAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string();
    let sol = "So11111111111111111111111111111111111111112".to_string();
    let usdc = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string();

    let pre = vec![
        TokenBalance { account_index: 0, mint: sol.clone(), owner: wallet.clone(), amount_raw: 3_000_000_000, ui_amount: 3.0 },
        TokenBalance { account_index: 1, mint: usdc.clone(), owner: wallet.clone(), amount_raw: 0, ui_amount: 0.0 },
    ];
    let post = vec![
        TokenBalance { account_index: 0, mint: sol, owner: wallet.clone(), amount_raw: 1_500_000_000, ui_amount: 1.5 },
        TokenBalance { account_index: 1, mint: usdc, owner: wallet, amount_raw: 200_000_000, ui_amount: 200.0 },
    ];
    (pre, post)
}

fn bench_build_swap_row(c: &mut Criterion) {
    let (pre, post) = sample_balances();

    c.bench_function("build_swap_row", |b| {
        b.iter(|| {
            build_swap_row(
                black_box("5xY...sig".to_string()),
                black_box("So1swapWa11etAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string()),
                black_box(250_000_000),
                black_box(Some(1_700_000_000)),
                black_box(&pre),
                black_box(&post),
            )
        })
    });
}

criterion_group!(benches, bench_build_swap_row);
criterion_main!(benches);
