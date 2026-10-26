use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use anyhow::anyhow;
use parser_raydium::RaydiumParser;
use parser_core::InstructionParser;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use types::{RawTx, TokenBalance};

enum Parser {
    Raydium(RaydiumParser),
}

const WSOL: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const RAYDIUM_AMM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

static PROGRAMS: LazyLock<HashMap<Pubkey, Parser>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert(
        RAYDIUM_AMM_V4,
        Parser::Raydium(RaydiumParser),
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
    let deltas = compute_deltas(&tx);
    for instr in &tx.instructions {
        let parsed_instr = match PROGRAMS.get(&instr.program_id) {
            Some(Parser::Raydium(parser)) => parser.parse(&instr)?,
            None => continue,
        };
        let operation = match parsed_instr {
            Some(op) => op,
            None => continue,
        };
    }
    Ok(())
}

struct Deltas {
    token_deltas: HashMap<Pubkey, Balance>,
    mints: HashMap<Pubkey, Pubkey>
}

fn compute_deltas(tx: &RawTx) -> Result<Deltas, anyhow::Error> {
    let wallet = tx.account_keys[0];
    let mut mints_by_account = HashMap::new();
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
        mints_by_account.insert(pre.account, pre.mint);
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
                mints_by_account.insert(post.account, post.mint);
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
    Ok(Deltas { token_deltas, mints: mints_by_account })
}
