//! Manually-triggered performance benchmark for the data layer.
//!
//! Measures the two things an indexer lives and dies by:
//!   1. write throughput  — how fast swaps can be upserted (rows/sec)
//!   2. query latency      — p50/p90/p99/max for the two API query patterns
//!
//! It runs the *same SQL the API runs*, directly against Postgres, so the numbers
//! isolate data-layer performance (index + planner + fetch) from HTTP overhead.
//! To load-test the HTTP surface too, point a tool like `oha`/`k6` at a running
//! `api` server once the DB is seeded.
//!
//! Run: `cargo run -p benchmarks --release --bin loadtest -- --rows 100000`
//!
//! WARNING: truncates the `swaps` table first for a clean, comparable baseline.

use std::time::Instant;

use clap::Parser;
use db::Swap;
use hdrhistogram::Histogram;

#[derive(Parser)]
#[command(about = "Benchmark write throughput and query latency against Postgres")]
struct Args {
    /// Rows to seed before measuring queries.
    #[arg(long, default_value_t = 100_000)]
    rows: usize,

    /// Query iterations per endpoint.
    #[arg(long, default_value_t = 1_000)]
    iterations: usize,

    /// Skip the TRUNCATE (measure against existing data instead of a clean slate).
    #[arg(long, default_value_t = false)]
    no_truncate: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    let args = Args::parse();

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?;
    let pool = db::connect(&database_url).await?;
    db::run_migrations(&pool).await?;

    if !args.no_truncate {
        println!("truncating swaps for a clean baseline...");
        sqlx::query("truncate table swaps").execute(&pool).await?;
    }

    // ---- Write throughput --------------------------------------------------
    println!("seeding {} rows...", args.rows);
    let started = Instant::now();
    let inserted = benchmarks::seed(&pool, args.rows, 0.2, "bench").await?;
    let write_elapsed = started.elapsed();
    let write_rps = inserted as f64 / write_elapsed.as_secs_f64();

    // ---- Query latency -----------------------------------------------------
    let hot_pair = benchmarks::hot_pair();

    // Warm up the pool / caches so the first query doesn't skew the histogram.
    let _ = wallet_query(&pool, benchmarks::HOT_WALLET).await?;
    let _ = pair_query(&pool, &hot_pair).await?;

    let wallet_hist = measure(args.iterations, || wallet_query(&pool, benchmarks::HOT_WALLET)).await?;
    let pair_hist = measure(args.iterations, || pair_query(&pool, &hot_pair)).await?;

    // ---- Report ------------------------------------------------------------
    println!("\n================ results ================");
    println!("dataset: {inserted} rows");
    println!(
        "\nwrite throughput: {:.0} rows/sec  ({} rows in {:.2}s)",
        write_rps,
        inserted,
        write_elapsed.as_secs_f64()
    );
    println!("\nquery latency ({} iterations each):", args.iterations);
    print_hist("  /swaps?wallet=", &wallet_hist);
    print_hist("  /swaps?pair=&since=", &pair_hist);
    println!("========================================");

    Ok(())
}

/// `select ... where wallet = $1 order by block_time desc limit 100` — identical
/// to the API handler.
async fn wallet_query(pool: &sqlx::PgPool, wallet: &str) -> anyhow::Result<usize> {
    let rows = sqlx::query_as::<_, Swap>(
        "select * from swaps where wallet = $1 order by block_time desc nulls last limit 100",
    )
    .bind(wallet)
    .fetch_all(pool)
    .await?;
    Ok(rows.len())
}

/// `select ... where pair = $1 and block_time >= $2 order by block_time desc limit 100`.
async fn pair_query(pool: &sqlx::PgPool, pair: &str) -> anyhow::Result<usize> {
    let epoch = chrono::DateTime::from_timestamp(0, 0).unwrap();
    let rows = sqlx::query_as::<_, Swap>(
        "select * from swaps where pair = $1 and block_time >= $2 order by block_time desc nulls last limit 100",
    )
    .bind(pair)
    .bind(epoch)
    .fetch_all(pool)
    .await?;
    Ok(rows.len())
}

/// Run `op` `iterations` times, recording each latency (in microseconds) into a
/// histogram.
async fn measure<F, Fut>(iterations: usize, mut op: F) -> anyhow::Result<Histogram<u64>>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<usize>>,
{
    let mut hist = Histogram::<u64>::new(3)?;
    for _ in 0..iterations {
        let t = Instant::now();
        op().await?;
        hist.record(t.elapsed().as_micros() as u64)?;
    }
    Ok(hist)
}

fn print_hist(label: &str, h: &Histogram<u64>) {
    println!(
        "{label:<24} p50={:>6}µs  p90={:>6}µs  p99={:>6}µs  max={:>6}µs  mean={:>6.0}µs",
        h.value_at_quantile(0.50),
        h.value_at_quantile(0.90),
        h.value_at_quantile(0.99),
        h.max(),
        h.mean(),
    );
}
