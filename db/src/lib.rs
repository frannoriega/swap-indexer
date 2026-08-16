//! Shared persistence layer: the `Swap` row type plus helpers to open a pool and
//! apply migrations. Kept deliberately thin — no query logic lives here; the
//! writer (`ingestion`) and reader (`api`) own their own SQL.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::postgres::{PgPool, PgPoolOptions};

/// One decoded swap. Column names match the `swaps` table (see migrations).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Swap {
    pub id: i64,
    pub signature: String,
    pub wallet: String,
    pub mint_in: String,
    pub mint_out: String,
    pub pair: String,
    pub amount_in: Decimal,
    pub amount_out: Decimal,
    pub price: Option<f64>,
    pub slot: i64,
    pub block_time: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Open a connection pool to Postgres.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

/// Apply pending migrations. Called by the ingestion service on startup — it is
/// the only writer, so it owns the schema. The API is read-only and assumes the
/// schema already exists.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
