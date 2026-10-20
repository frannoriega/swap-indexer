pub mod process;

use anyhow::anyhow;
use clap::{Arg, Command, ValueEnum, value_parser};
use rpc_fetcher::RpcFetcher;
use solana_sdk::signature::Signature;
use types::Fetcher;

use crate::process::process;

#[derive(Copy, Clone, ValueEnum)]
enum FetcherOpt {
    #[value(alias = "rpc")]
    Rpc,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let matches = Command::new("Swap indexer")
        .version("0.1.0")
        .arg(
            Arg::new("url")
                .required(true)
                .help("Set the URL for the node")
                .ignore_case(true),
        )
        .arg(
            Arg::new("signature")
                .required(true)
                .help("Transaction signature")
                .value_parser(value_parser!(Signature))
                .ignore_case(true),
        )
        .arg(
            Arg::new("fetcher")
                .required(true)
                .help("Choose the fetcher to use")
                .value_parser(value_parser!(FetcherOpt))
                .ignore_case(true),
        )
        .get_matches();

    let url = matches
        .get_one::<String>("url")
        .ok_or_else(|| anyhow!("URL not set"))?;
    let signature = matches
        .get_one::<Signature>("signature")
        .ok_or_else(|| anyhow!("signature not set"))?;
    let fetcher_opt = matches
        .get_one::<FetcherOpt>("fetcher")
        .ok_or_else(|| anyhow!("fetcher not set"))?;

    let fetcher = match fetcher_opt {
        FetcherOpt::Rpc => RpcFetcher::new(url.to_owned()),
    };
    let tx = fetcher.get_transaction(signature).await?;
    let result = process(tx);
    Ok(())
}
