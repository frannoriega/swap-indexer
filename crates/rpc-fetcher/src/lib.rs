use std::str::FromStr;

use anyhow::anyhow;
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_config::{CommitmentConfig, CommitmentLevel, RpcTransactionConfig, UiTransactionEncoding},
    rpc_response::{EncodedTransactionWithStatusMeta, UiInstruction, UiTransactionTokenBalance},
};
use solana_sdk::{bs58, pubkey::Pubkey, signature::Signature, transaction::TransactionError};
use types::{Fetcher, Instruction, RawTx, TokenBalance};

/**
 * Decodes the Base58 data present in inner instructions to bytes
 */
fn decode_inner(data: String) -> Result<Vec<u8>, anyhow::Error> {
    bs58::decode(data)
        .into_vec()
        .map_err(|_| anyhow!("unable to decode inner instruction data"))
}

/**
 * Helper method to search the account keys based on the indices.
 */
fn get_accounts(account_keys: &[Pubkey], indices: &[u8]) -> Result<Vec<Pubkey>, anyhow::Error> {
    let mut accounts = Vec::new();
    for idx in indices {
        accounts.push(
            *account_keys
                .get(*idx as usize)
                .ok_or_else(|| anyhow!("index not found in account keys"))?,
        )
    }
    Ok(accounts)
}

fn resolve_token_balances(
    account_keys: &[Pubkey],
    balances: Vec<UiTransactionTokenBalance>,
) -> Result<Vec<TokenBalance>, anyhow::Error> {
    balances
        .into_iter()
        .map(|b| {
            Ok(TokenBalance {
                account: *account_keys
                    .get(b.account_index as usize)
                    .ok_or_else(|| anyhow!("index not found in account keys"))?,
                mint: Pubkey::from_str(&b.mint).map_err(|_| anyhow!("malformed mint"))?,
                owner: b
                    .owner
                    .as_ref()
                    .map(|o| Pubkey::from_str(o))
                    .transpose()
                    .map_err(|_| anyhow!("malformed owner"))?,
                amount: b.ui_token_amount.amount.parse()?,
                decimals: b.ui_token_amount.decimals,
            })
        })
        .collect()
}

/**
 * The transaction data after resolving indices and addresses.
 * Instructions here are flattened (top-level and inner instructions)
 */
struct ResolvedTransaction {
    account_keys: Vec<Pubkey>,
    instructions: Vec<Instruction>,
    status: Option<TransactionError>,
    fee: u64,
    pre_balances: Vec<u64>,
    post_balances: Vec<u64>,
    pre_token_balances: Vec<TokenBalance>,
    post_token_balances: Vec<TokenBalance>,
}

/**
 * Resolves the accounts, instructions and status from a Solana's SDK
 * [EncodedTransactionWithStatusMeta]
 */
fn resolve_transaction(
    tx: EncodedTransactionWithStatusMeta,
) -> Result<ResolvedTransaction, anyhow::Error> {
    // Check that we have all the data we want at the beginning. Fail fast
    let meta = tx
        .meta
        .ok_or_else(|| anyhow!("transaction missing metadata"))?;
    let inner_instrs = meta
        .inner_instructions
        .ok_or_else(|| anyhow!("metadata missing inner instructions"))?;
    let decoded_tx = tx
        .transaction
        .decode()
        .ok_or_else(|| anyhow!("no transaction"))?;
    let loaded_addresses = meta
        .loaded_addresses
        .ok_or_else(|| anyhow!("missing loaded addresses"))?;
    let message = decoded_tx.message;
    // Build the account keys, in order: static keys + writable + readonly
    let mut account_keys = Vec::new();
    for static_addr in message.static_account_keys() {
        account_keys.push(*static_addr);
    }
    for writable in loaded_addresses.writable {
        account_keys
            .push(Pubkey::from_str(&writable).map_err(|_| anyhow!("malformed writable address"))?);
    }
    for readonly in loaded_addresses.readonly {
        account_keys
            .push(Pubkey::from_str(&readonly).map_err(|_| anyhow!("malformed readonly address"))?)
    }
    // Build the instructions
    let mut instructions = Vec::new();
    for (instr_idx, instr) in message.instructions().iter().enumerate() {
        instructions.push(Instruction {
            program_id: account_keys[instr.program_id_index as usize],
            accounts: get_accounts(&account_keys, &instr.accounts)?,
            stack_depth: 0, // 0 = top level
            outer_idx: instr_idx as u8,
            data: instr.data.clone(),
        });
    }
    for inner_instr in inner_instrs {
        let tx_idx = inner_instr.index;
        for instr in inner_instr.instructions {
            match instr {
                UiInstruction::Compiled(compiled_instr) => {
                    instructions.push(Instruction {
                        program_id: *account_keys
                            .get(compiled_instr.program_id_index as usize)
                            .ok_or_else(|| anyhow!("index not found in account keys"))?,
                        accounts: get_accounts(&account_keys, &compiled_instr.accounts)?,
                        stack_depth: compiled_instr
                            .stack_height
                            .ok_or_else(|| anyhow!("inner instruction missing depth"))?,
                        data: decode_inner(compiled_instr.data)?,
                        outer_idx: tx_idx,
                    });
                }
                UiInstruction::Parsed(_) => {
                    return Err(anyhow!("inner instruction data in wrong shape"));
                }
            }
        }
    }
    Ok(ResolvedTransaction {
        instructions,
        status: meta.err.map(TransactionError::from),
        fee: meta.fee,
        pre_balances: meta.pre_balances,
        post_balances: meta.post_balances,
        pre_token_balances: resolve_token_balances(
            &account_keys,
            meta.pre_token_balances
                .ok_or_else(|| anyhow!("Pre-token balances not available"))?,
        )?,
        post_token_balances: resolve_token_balances(
            &account_keys,
            meta.post_token_balances
                .ok_or_else(|| anyhow!("Pre-token balances not available"))?,
        )?,
        account_keys
    })
}

/**
 * [Fetcher] implementation using Solana's [RpcClient]
 */
pub struct RpcFetcher {
    client: RpcClient,
}

impl RpcFetcher {
    pub fn new(url: String) -> RpcFetcher {
        RpcFetcher {
            client: RpcClient::new(url),
        }
    }
}

impl Fetcher for RpcFetcher {
    async fn get_transaction(&self, signature: &Signature) -> Result<RawTx, anyhow::Error> {
        let response = self
            .client
            .get_transaction_with_config(
                signature,
                RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Base64),
                    commitment: Some(CommitmentConfig {
                        commitment: CommitmentLevel::Confirmed,
                    }),
                    max_supported_transaction_version: Some(0),
                },
            )
            .await
            .map_err(|_| anyhow::Error::msg("error retrieving transaction"))?;
        let resolved_tx = resolve_transaction(response.transaction)?;
        let slot = response.slot;
        let block_time = response.block_time;

        Ok(RawTx {
            signature: *signature,
            slot,
            block_time,
            account_keys: resolved_tx.account_keys,
            instructions: resolved_tx.instructions,
            status: resolved_tx.status,

            fee: resolved_tx.fee,
            pre_balances: resolved_tx.pre_balances,
            post_balances: resolved_tx.post_balances,
            pre_token_balances: resolved_tx.pre_token_balances,
            post_token_balances: resolved_tx.post_token_balances
        })
    }
}
