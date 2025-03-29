use std::{env, path::Path};
use std::str::FromStr;
use solana_sdk::{
    signature::{ Signer, read_keypair_file},
    transaction::Transaction,
    system_instruction,
    commitment_config::CommitmentConfig,
};
use solana_client::rpc_client::RpcClient;
use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let merkle_distributor_program_id = Pubkey::from_str("MRKGLMizK9XSTaD1d1jbVkdHZbQVCSnPpYiTw9aKQv8")?;

    if args.len() < 2 || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        std::process::exit(0);
    }
    let (rpc_url, keypair_path) = parse_args(&args)?;

    let payer = match read_keypair_file(Path::new(&keypair_path)) {
        Ok(payer) => payer,
        Err(err) => {
            anyhow::bail!("Failed to read keypair file: {}", err);
        }
    };

    let client = RpcClient::new_with_commitment(
        rpc_url,
        CommitmentConfig::confirmed(),
    );
    check_airdrop(&client, &payer, &merkle_distributor_program_id)?;
    claim_airdrop(&client, &payer, &merkle_distributor_program_id)?;

    Ok(())
}

#[derive(Debug, Deserialize)]
struct AirdropProof {
    index: u64,
    amount: u64,
    proof: Vec<String>,
}

fn check_airdrop(client: &RpcClient, payer: &dyn Signer, program_id: &Pubkey) -> Result<()> {
    let key = payer.pubkey();
    let url = format!(
        "https://solanavault.github.io/simd-228-vpts-airdrop/proofs/{}.json",
        key
    );

    let response = Client::new().get(&url).send()?;
    if response.status().is_success() {
        let proof: AirdropProof = response.json()?;
        println!("Airdrop found: {:?}", proof);

        let distributor = Pubkey::from_str("BAzvEJH5w7igbbkiRLhCD89b1gLtrZz6B9wxxuh3ocJz")?;
        let claim_status_key = find_claim_status_key(proof.index, &distributor, program_id);
        println!("Claim status key: {}", claim_status_key );

        let account = client.get_account(&claim_status_key);
        if account.is_ok() {
            println!("Claim already made.");
        } else {
            println!("{:?} point Airdrop is available to claim.", proof.amount/1_000_000);
        }
    } else if response.status().as_u16() == 404 {
        println!("No airdrop available.");
    } else {
        return Err(anyhow!("Failed to fetch airdrop info: {}", response.status()));
    }
    Ok(())
}

fn find_claim_status_key(index: u64, distributor: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let seeds: &[&[u8]] = &[
        b"ClaimStatus",
        &index.to_le_bytes(),         // index is a u64
        distributor.as_ref(),         // distributor is a &Pubkey
    ];
    Pubkey::find_program_address(seeds, program_id).0
}

fn claim_airdrop(client: &RpcClient, payer: &dyn Signer, program_id: &Pubkey) -> Result<()> {
    let to = payer.pubkey();
    let ix = system_instruction::transfer(&to, &to, 1_000_000); // dummy claim
    let blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&to),
        &[payer],
        blockhash,
    );

    let sig = client.send_and_confirm_transaction(&tx)?;

    println!("✅ Transaction sent!");
    println!("Signature: {}", sig);

    Ok(())
}

fn parse_args(args: &[String]) -> Result<(String, String)> {
    let mut rpc_url = "https://api.mainnet-beta.solana.com".to_string();
    let mut keypair_path = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-u" => {
                i += 1;
                if i >= args.len() {
                    anyhow::bail!("Missing value for -u");
                }
                rpc_url = args[i].clone();
            },
            arg if !arg.starts_with('-') => {
                if keypair_path.is_some() {
                    anyhow::bail!("Unexpected extra argument: {}", arg);
                }
                keypair_path = Some(arg.to_string());
            },
            flag => anyhow::bail!("Unknown flag: {}", flag),
        }
        i += 1;
    }

    let keypair_path = keypair_path.ok_or_else(|| anyhow::anyhow!("Missing keypair path"))?;
    Ok((rpc_url, keypair_path))
}

fn print_help() {
    println!(
        "Usage:
  cargo run -- <KEYPAIR_PATH> [-u <RPC_URL>]

Arguments:
  <KEYPAIR_PATH>   Path to the Solana keypair file.

Options:
  -u <RPC_URL>     Override the default RPC URL (default: https://api.mainnet-beta.solana.com)
  -h, --help       Show this help message
"
    );
}
