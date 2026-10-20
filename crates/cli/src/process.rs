use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use anyhow::anyhow;
use parser_core::TxParser;
use parser_raydium::{RaydiumParser, instruction::AmmInstruction};
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use types::{RawTx, TokenBalance};

enum Parser {
    Raydium(TxParser<RaydiumParser, AmmInstruction>),
}

const WSOL: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const RAYDIUM_AMM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

static PROGRAMS: LazyLock<HashMap<Pubkey, Parser>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert(
        RAYDIUM_AMM_V4,
        Parser::Raydium(TxParser::new(RaydiumParser)),
    );
    map
});

struct Balance {
    amount: i128,
    decimals: u8,
}

pub fn process(tx: RawTx) -> Result<(), anyhow::Error> {
    if let Some(err) = tx.status {
        return Err(anyhow!("Transaction failed: {:}", err));
    }
    let token_deltas = compute_deltas(&tx);
    Ok(())
}

fn compute_deltas(tx: &RawTx) -> Result<HashMap<Pubkey, Balance>, anyhow::Error> {
    let wallet = tx.account_keys[0];
    let mut token_deltas = HashMap::new();
    token_deltas.insert(
        WSOL,
        Balance {
            amount: tx.post_balances[0] as i128 - tx.pre_balances[0] as i128 + tx.fee as i128,
            // Known SOL decimals
            decimals: 9u8,
        },
    );
    let mut pre_token_balances = HashMap::new();
    for pre in tx.pre_token_balances.iter() {
        if pre_token_balances.contains_key(&pre.account) {
            return Err(anyhow!(
                "malformed rpc response: repeated account in pre token balances"
            ));
        }
        pre_token_balances.insert(
            pre.account,
            TokenBalance {
                account: pre.account,
                mint: pre.mint,
                owner: pre.owner,
                amount: pre.amount,
                decimals: pre.decimals,
            },
        );
    }
    {
        // This is just to ensure and guard against duplicated addresses.
        // Such scenario means a malformed RPC response.
        // Dropped at the end of this scope
        let mut post_token_accounts = HashSet::new();
        for post in tx.post_token_balances.iter() {
            if post.owner == Some(wallet) {
                if !post_token_accounts.insert(post.account) {
                    return Err(anyhow!(
                        "malformed rpc response: repeated account in post token balances"
                    ));
                }
                match pre_token_balances.remove(&post.account) {
                    Some(pre) => {
                        token_deltas
                            .entry(post.mint)
                            .or_insert(Balance {
                                amount: 0,
                                decimals: post.decimals,
                            })
                            .amount += post.amount as i128 - pre.amount as i128
                    }
                    None => {
                        token_deltas
                            .entry(post.mint)
                            .or_insert(Balance {
                                amount: 0,
                                decimals: post.decimals,
                            })
                            .amount += post.amount as i128
                    }
                };
            }
        }
    }
    for (_, leftover) in pre_token_balances.drain() {
        if leftover.owner == Some(wallet) {
            token_deltas
                .entry(leftover.mint)
                .or_insert(Balance {
                    amount: 0,
                    decimals: leftover.decimals,
                })
                .amount -= leftover.amount as i128;
        }
    }
    // Remove movements in which the delta is zero
    // That's just noise
    token_deltas.retain(|_, v| v.amount != 0);
    Ok(token_deltas)
}
