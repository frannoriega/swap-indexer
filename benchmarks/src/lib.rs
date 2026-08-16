//! Synthetic swap generation + bulk seeding, shared by the `seed` and `loadtest`
//! binaries. None of this ships in the indexer — it exists so you can populate a
//! local database with realistic-shaped data without a live Yellowstone stream.

use chrono::{Duration, Utc};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rust_decimal::Decimal;
use sqlx::{PgPool, QueryBuilder};

/// A known wallet and pair a large fraction of the synthetic rows are attributed
/// to, so the indexed queries return meaningful (up to the 100-row limit) result
/// sets. The `seed` binary prints these so you can curl the API directly.
pub const HOT_WALLET: &str = "BenchHotWa11et1111111111111111111111111111111";
pub const HOT_MINT_A: &str = "So11111111111111111111111111111111111111112"; // SOL
pub const HOT_MINT_B: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"; // USDC

const BATCH: usize = 1_000; // rows per multi-row insert (10 cols -> 10k binds)

/// Canonical pair key — mirrors `ingestion::mapping::canonical_pair`. Duplicated
/// (one line) so this dev crate doesn't pull in Carbon/Solana just to sort two
/// strings.
pub fn canonical_pair(a: &str, b: &str) -> String {
    if a <= b {
        format!("{a}-{b}")
    } else {
        format!("{b}-{a}")
    }
}

/// The canonical key for the hot pair; use this to build `?pair=` queries.
pub fn hot_pair() -> String {
    canonical_pair(HOT_MINT_A, HOT_MINT_B)
}

struct Row {
    signature: String,
    wallet: String,
    mint_in: String,
    mint_out: String,
    pair: String,
    amount_in: Decimal,
    amount_out: Decimal,
    price: f64,
    slot: i64,
    block_time: chrono::DateTime<Utc>,
}

fn rand_base58(rng: &mut StdRng, len: usize) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    (0..len)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Insert `rows` synthetic swaps. `hot_fraction` of them are attributed to the
/// hot wallet and hot pair; the rest are spread over a pool of random wallets and
/// mints to give the indexes realistic cardinality. Returns the number inserted.
///
/// `run_prefix` keeps signatures unique across repeated runs against the same DB.
pub async fn seed(
    pool: &PgPool,
    rows: usize,
    hot_fraction: f64,
    run_prefix: &str,
) -> anyhow::Result<usize> {
    // Deterministic RNG so runs are reproducible.
    let mut rng = StdRng::seed_from_u64(0xC0FFEE);

    // Pools sized so many rows share a wallet/mint (realistic cardinality).
    let wallet_pool: Vec<String> = (0..(rows / 50).max(1))
        .map(|_| rand_base58(&mut rng, 44))
        .collect();
    let mint_pool: Vec<String> = (0..(rows / 200).max(2))
        .map(|_| rand_base58(&mut rng, 44))
        .collect();

    let now = Utc::now();
    let mut buffer: Vec<Row> = Vec::with_capacity(BATCH);
    let mut inserted = 0;

    for i in 0..rows {
        let hot = rng.gen_bool(hot_fraction);

        let (wallet, mint_in, mint_out) = if hot {
            (
                HOT_WALLET.to_string(),
                HOT_MINT_A.to_string(),
                HOT_MINT_B.to_string(),
            )
        } else {
            let a = mint_pool[rng.gen_range(0..mint_pool.len())].clone();
            let mut b = mint_pool[rng.gen_range(0..mint_pool.len())].clone();
            if b == a {
                b = mint_pool[(rng.gen_range(0..mint_pool.len()) + 1) % mint_pool.len()].clone();
            }
            (wallet_pool[rng.gen_range(0..wallet_pool.len())].clone(), a, b)
        };

        buffer.push(Row {
            signature: format!("{run_prefix}-{i}"),
            pair: canonical_pair(&mint_in, &mint_out),
            wallet,
            mint_in,
            mint_out,
            amount_in: Decimal::from(rng.gen_range(1u64..1_000_000_000)),
            amount_out: Decimal::from(rng.gen_range(1u64..1_000_000_000)),
            price: rng.gen_range(0.0001..10_000.0),
            slot: 300_000_000 + i as i64,
            block_time: now - Duration::seconds(rng.gen_range(0..2_592_000)), // within ~30d
        });

        if buffer.len() == BATCH {
            inserted += flush(pool, &buffer).await?;
            buffer.clear();
        }
    }
    if !buffer.is_empty() {
        inserted += flush(pool, &buffer).await?;
    }

    Ok(inserted)
}

async fn flush(pool: &PgPool, rows: &[Row]) -> anyhow::Result<usize> {
    let mut qb = QueryBuilder::new(
        "insert into swaps (signature, wallet, mint_in, mint_out, pair, amount_in, amount_out, price, slot, block_time) ",
    );
    qb.push_values(rows, |mut b, row| {
        b.push_bind(&row.signature)
            .push_bind(&row.wallet)
            .push_bind(&row.mint_in)
            .push_bind(&row.mint_out)
            .push_bind(&row.pair)
            .push_bind(row.amount_in)
            .push_bind(row.amount_out)
            .push_bind(row.price)
            .push_bind(row.slot)
            .push_bind(row.block_time);
    });
    qb.push(" on conflict (signature) do nothing");
    let result = qb.build().execute(pool).await?;
    Ok(result.rows_affected() as usize)
}
