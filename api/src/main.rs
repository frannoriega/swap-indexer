//! Read-only API over the `swaps` table.
//!
//! Two query patterns, matching the two indexes:
//!   GET /swaps?wallet=<pubkey>
//!   GET /swaps?pair=<mint_a>-<mint_b>&since=<unix_seconds>
//!
//! There are no write endpoints — the ingestion service is the only writer.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use chrono::{DateTime, Utc};
use db::Swap;
use serde::Deserialize;
use sqlx::PgPool;

const MAX_ROWS: i64 = 100;

#[derive(Debug, Deserialize)]
struct SwapsQuery {
    wallet: Option<String>,
    /// "<mint_a>-<mint_b>" — order doesn't matter; normalized server-side.
    pair: Option<String>,
    /// Unix seconds; only swaps at or after this block time are returned.
    since: Option<i64>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?;
    let pool = db::connect(&database_url).await?;

    let app = Router::new()
        .route("/swaps", get(get_swaps))
        .with_state(pool);

    let addr = std::env::var("API_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    log::info!("API listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}

/// `GET /swaps` — exactly one of `wallet` or `pair` must be supplied.
async fn get_swaps(
    State(pool): State<PgPool>,
    Query(q): Query<SwapsQuery>,
) -> Result<Json<Vec<Swap>>, (StatusCode, String)> {
    if let Some(wallet) = q.wallet {
        let rows = sqlx::query_as::<_, Swap>(
            "select * from swaps
             where wallet = $1
             order by block_time desc nulls last
             limit $2",
        )
        .bind(wallet)
        .bind(MAX_ROWS)
        .fetch_all(&pool)
        .await
        .map_err(internal)?;
        return Ok(Json(rows));
    }

    if let Some(pair) = q.pair {
        // Normalize to the canonical, order-independent pair key the rows are
        // stored under (min-max of the two mints). Mints are base58, so '-' is a
        // safe separator.
        let mut mints: Vec<&str> = pair.split('-').collect();
        if mints.len() != 2 || mints.iter().any(|m| m.is_empty()) {
            return Err((
                StatusCode::BAD_REQUEST,
                "pair must be '<mint_a>-<mint_b>'".to_string(),
            ));
        }
        mints.sort_unstable();
        let canonical = format!("{}-{}", mints[0], mints[1]);

        // `since` defaults to the epoch so `?pair=` alone returns full history.
        let since = DateTime::<Utc>::from_timestamp(q.since.unwrap_or(0), 0)
            .ok_or((StatusCode::BAD_REQUEST, "invalid 'since' timestamp".to_string()))?;

        let rows = sqlx::query_as::<_, Swap>(
            "select * from swaps
             where pair = $1 and block_time >= $2
             order by block_time desc nulls last
             limit $3",
        )
        .bind(canonical)
        .bind(since)
        .bind(MAX_ROWS)
        .fetch_all(&pool)
        .await
        .map_err(internal)?;
        return Ok(Json(rows));
    }

    Err((
        StatusCode::BAD_REQUEST,
        "provide ?wallet=<pubkey> or ?pair=<mint_a>-<mint_b>[&since=<unix_seconds>]".to_string(),
    ))
}

fn internal(e: sqlx::Error) -> (StatusCode, String) {
    log::error!("query failed: {e}");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string())
}
