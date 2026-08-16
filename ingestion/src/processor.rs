//! Maps a decoded Raydium AMM V4 swap instruction to a `swaps` row and upserts it.
//!
//! The processor stays thin: pull the wallet + token balances out of the Carbon
//! input, hand them to the pure [`crate::mapping`] logic, and upsert the result.
//! No business logic beyond that in v1.

use carbon_core::{
    error::CarbonResult, instruction::InstructionProcessorInputType, processor::Processor,
};
use carbon_raydium_amm_v4_decoder::instructions::RaydiumAmmV4Instruction;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::mapping::{build_swap_row, TokenBalance};

pub struct SwapProcessor {
    pub pool: PgPool,
}

impl Processor<InstructionProcessorInputType<'_, RaydiumAmmV4Instruction>> for SwapProcessor {
    async fn process(
        &mut self,
        input: &InstructionProcessorInputType<'_, RaydiumAmmV4Instruction>,
    ) -> CarbonResult<()> {
        let tx = &input.metadata.transaction_metadata;

        // Only the swap variants matter; ignore deposits, withdrawals, admin ops.
        // v1 handles the two canonical swaps; the `V2` variants can be added the
        // same way once needed (kept out to keep the POC narrow).
        let wallet = match &input.decoded_instruction {
            RaydiumAmmV4Instruction::SwapBaseIn { accounts, .. } => accounts.user_source_owner,
            RaydiumAmmV4Instruction::SwapBaseOut { accounts, .. } => accounts.user_source_owner,
            _ => return Ok(()),
        };
        let wallet = wallet.to_string();

        let pre = to_balances(tx.meta.pre_token_balances.as_deref());
        let post = to_balances(tx.meta.post_token_balances.as_deref());

        let Some(row) = build_swap_row(
            tx.signature.to_string(),
            wallet,
            tx.slot,
            tx.block_time,
            &pre,
            &post,
        ) else {
            // No wallet-owned in/out pair in the balances — skip rather than guess.
            log::warn!("sig={} could not resolve swap legs; skipping", tx.signature);
            return Ok(());
        };

        let block_time = row
            .block_time
            .and_then(|t| DateTime::<Utc>::from_timestamp(t, 0));

        // Upsert by signature — the idempotency key. `do nothing` because the row
        // is fully determined by the signature; re-seeing the tx must not dup it.
        let result = sqlx::query(
            "insert into swaps
                 (signature, wallet, mint_in, mint_out, pair, amount_in, amount_out, price, slot, block_time)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             on conflict (signature) do nothing",
        )
        .bind(&row.signature)
        .bind(&row.wallet)
        .bind(&row.mint_in)
        .bind(&row.mint_out)
        .bind(&row.pair)
        .bind(Decimal::from(row.amount_in))
        .bind(Decimal::from(row.amount_out))
        .bind(row.price)
        .bind(row.slot)
        .bind(block_time)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => log::info!(
                "swap sig={} wallet={} {} {} -> {} {}",
                row.signature,
                row.wallet,
                row.amount_in,
                row.mint_in,
                row.amount_out,
                row.mint_out
            ),
            // Log and continue — one bad row must not tear down the whole stream.
            Err(e) => log::error!("sig={} upsert failed: {e}", row.signature),
        }

        Ok(())
    }
}

/// Flatten Carbon's token balances into the DB-agnostic [`TokenBalance`] the
/// mapping logic consumes. `ui_token_amount.amount` is a base-unit string.
fn to_balances(
    balances: Option<&[solana_transaction_status::TransactionTokenBalance]>,
) -> Vec<TokenBalance> {
    balances
        .unwrap_or(&[])
        .iter()
        .map(|b| TokenBalance {
            account_index: b.account_index,
            mint: b.mint.clone(),
            owner: b.owner.clone(),
            amount_raw: b.ui_token_amount.amount.parse().unwrap_or(0),
            ui_amount: b.ui_token_amount.ui_amount.unwrap_or(0.0),
        })
        .collect()
}
