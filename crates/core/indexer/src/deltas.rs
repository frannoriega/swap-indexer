use std::collections::{HashMap, HashSet};

use anyhow::anyhow;

use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use types::{RawTx, TokenBalance};

const WSOL: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

pub struct Balance {
    pub amount: i128,
    pub decimals: u8,
}

pub struct Deltas {
    pub token_deltas: HashMap<Pubkey, Balance>,
    pub mints: HashMap<Pubkey, Pubkey>,
}

pub fn compute_deltas(tx: &RawTx) -> Result<Deltas, anyhow::Error> {
    let wallet = tx.account_keys[0];
    let mut mints_by_account = HashMap::new();
    // We need to join the accounts in pre and post token balances.
    // We only need the accounts owned by the signer (`wallet`)
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

    // Iterate the pre token balances and store:
    // 1. The token associated with their account
    // 2. The token balance of their account
    for pre in tx.pre_token_balances.iter() {
        // 1. Store the token associated with the account
        mints_by_account.insert(pre.account, pre.mint);
        // Guard against duplicated addresses.
        // Such scenario means a malformed RPC response.
        if pre_token_balances.contains_key(&pre.account) {
            return Err(anyhow!(
                "malformed rpc response: repeated account in pre token balances"
            ));
        }
        // 2. Store the balance of the account
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
        // We will now be using the pre_token_balances for the join,
        // so here we will make use of the `owner` field.
        // We only care about token balances owned by the signer (`wallet`)
        for post in tx.post_token_balances.iter() {
            if post.owner == Some(wallet) {
                // Store the mint address associated with the account
                mints_by_account.insert(post.account, post.mint);
                if !post_token_accounts.insert(post.account) {
                    return Err(anyhow!(
                        "malformed rpc response: repeated account in post token balances"
                    ));
                }
                match pre_token_balances.remove(&post.account) {
                    Some(pre) => {
                        // If a `post` account shows up in the `pre_token_balances` as
                        // well, then compute the difference (post - pre) and store it
                        // under the account token balance
                        token_deltas
                            .entry(post.mint)
                            .or_insert(Balance {
                                amount: 0,
                                decimals: post.decimals,
                            })
                            .amount += post.amount as i128 - pre.amount as i128
                    },
                    None => {
                        // If a `post` account shows up only in `post_balance_tokens`, that means
                        // the account was created mid-transaction
                        token_deltas
                            .entry(post.mint)
                            .or_insert(Balance {
                                amount: 0,
                                decimals: post.decimals,
                            })
                            .amount += post.amount as i128
                    },
                };
            }
        }
    }
    // Now there's a case where some accounts could be closed mid-transaction.
    // If that happened, then those accounts would not be present in `post_token_balances`.
    // So, we iterate whatever accounts were left in `pre_token_balances` and we compute the deltas
    // for those that are owned by the signer (`wallet`)
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
    Ok(Deltas {
        token_deltas,
        mints: mints_by_account,
    })
}
