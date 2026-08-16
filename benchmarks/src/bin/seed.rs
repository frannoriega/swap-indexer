//! Populate Postgres with synthetic swaps so you can exercise the API by hand
//! without a live stream.
//!
//! Run: `cargo run -p benchmarks --bin seed -- --rows 5000`

use clap::Parser;

#[derive(Parser)]
#[command(about = "Seed the swaps table with synthetic data for manual API testing")]
struct Args {
    /// Number of rows to insert.
    #[arg(long, default_value_t = 5_000)]
    rows: usize,

    /// Fraction attributed to the well-known hot wallet / pair (0.0–1.0).
    #[arg(long, default_value_t = 0.2)]
    hot_fraction: f64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    let args = Args::parse();

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?;
    let pool = db::connect(&database_url).await?;
    db::run_migrations(&pool).await?;

    // Unique prefix per run so repeated seeding doesn't collide on signature.
    let run_prefix = format!("seed-{}", chrono::Utc::now().timestamp());
    let inserted = benchmarks::seed(&pool, args.rows, args.hot_fraction, &run_prefix).await?;

    println!("inserted {inserted} rows");
    println!();
    println!("try the API against this data:");
    println!("  curl 'http://localhost:3000/swaps?wallet={}'", benchmarks::HOT_WALLET);
    println!(
        "  curl 'http://localhost:3000/swaps?pair={}-{}&since=0'",
        benchmarks::HOT_MINT_A,
        benchmarks::HOT_MINT_B
    );

    Ok(())
}
