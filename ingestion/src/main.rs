//! Ingestion service.
//!
//! Subscribes to a Yellowstone gRPC endpoint, filtered to the Raydium AMM V4
//! program, decodes swap instructions with Carbon, and upserts each one into
//! Postgres. This binary is the *only* writer of the `swaps` table.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use carbon_core::pipeline::{Pipeline, ShutdownStrategy};
use carbon_log_metrics::LogMetrics;
use carbon_raydium_amm_v4_decoder::{RaydiumAmmV4Decoder, PROGRAM_ID};
use carbon_yellowstone_grpc_datasource::{YellowstoneGrpcClientConfig, YellowstoneGrpcGeyserClient};
use tokio::sync::RwLock;
use yellowstone_grpc_proto::geyser::{CommitmentLevel, SubscribeRequestFilterTransactions};

use ingestion::processor::SwapProcessor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    // Carbon's gRPC transport uses rustls; a crypto provider must be installed
    // once, before any TLS connection is opened.
    // https://github.com/rustls/rustls/issues/1877
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    // Persistence: connect and apply migrations. Ingestion owns the schema.
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?;
    let pool = db::connect(&database_url).await?;
    db::run_migrations(&pool).await?;
    log::info!("connected to Postgres and applied migrations");

    // Subscribe only to transactions that touch the target program. `account_required`
    // means "every returned tx must reference this account" — i.e. the Raydium AMM V4
    // program id. Skip votes and failed txs; a failed swap never moved funds.
    let mut transaction_filters = HashMap::new();
    transaction_filters.insert(
        "raydium_amm_v4".to_string(),
        SubscribeRequestFilterTransactions {
            vote: Some(false),
            failed: Some(false),
            account_include: vec![],
            account_exclude: vec![],
            account_required: vec![PROGRAM_ID.to_string()],
            signature: None,
        },
    );

    let datasource = YellowstoneGrpcGeyserClient::new(
        std::env::var("YELLOWSTONE_GRPC_URL")
            .map_err(|_| anyhow::anyhow!("YELLOWSTONE_GRPC_URL must be set"))?,
        std::env::var("YELLOWSTONE_X_TOKEN").ok(),
        Some(CommitmentLevel::Confirmed),
        HashMap::default(), // no account subscriptions — we only want transactions
        transaction_filters,
        Default::default(),
        Arc::new(RwLock::new(HashSet::new())),
        YellowstoneGrpcClientConfig::default(),
        None,
        None,
    );

    log::info!("starting pipeline; filtering to program {PROGRAM_ID}");

    Pipeline::builder()
        .datasource(datasource)
        .metrics(Arc::new(LogMetrics::new()))
        .instruction(RaydiumAmmV4Decoder, SwapProcessor { pool })
        .shutdown_strategy(ShutdownStrategy::Immediate)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build pipeline: {e}"))?
        .run()
        .await
        .map_err(|e| anyhow::anyhow!("pipeline error: {e}"))?;

    Ok(())
}
