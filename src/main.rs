use std::{env, path::Path};
use std::rc::Rc;
use std::str::FromStr;
use anchor_client::Cluster;
use solana_sdk::signature::{ Signer, read_keypair_file};
use solana_client::rpc_client::RpcClient;
use anyhow::{anyhow, Result};
use dotenvy::dotenv;
use reqwest::blocking::Client;
use serde::Deserialize;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use spl_token::solana_program::pubkey::Pubkey as SplPubkey;
use serde_with::{serde_as, DisplayFromStr};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let merkle_distributor_program_id = Pubkey::from_str("MRKGLMizK9XSTaD1d1jbVkdHZbQVCSnPpYiTw9aKQv8")?;
    dotenv().ok();
    let distributor = Pubkey::from_str(&env::var("DISTRIBUTOR_ADDRESS")?)?;


    if args.len() < 2 || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        std::process::exit(0);
    }
    let (rpc_url, keypair_path) = parse_args(&args)?;

    let payer = match read_keypair_file(Path::new(&keypair_path)) {
        Ok(payer) => Rc::from(payer),
        Err(err) => {
            anyhow::bail!("Failed to read keypair file: {}", err);
        }
    };

    let client = RpcClient::new( rpc_url);
    let info = check_airdrop(&client, &(*payer).pubkey(), &distributor, &merkle_distributor_program_id)?;
    claim_airdrop(&client, payer, distributor, info, &merkle_distributor_program_id)?;

    Ok(())
}
#[serde_as]
#[derive(Debug, Deserialize)]
struct AirdropProof {
    index: u64,
    #[serde_as(as = "DisplayFromStr")]
    amount: u64,
    proof: Vec<String>,
}

fn spl_to_sdk(pubkey: &SplPubkey) -> Pubkey {
    let pubkey_array : [u8; 32] = pubkey.to_bytes();
    Pubkey::new_from_array(pubkey_array)
}

fn sdk_to_spl(pubkey: &Pubkey) -> SplPubkey {
    let pubkey_array = pubkey.to_bytes();
    SplPubkey::new_from_array(pubkey_array)
}

fn create_token_account(client: &RpcClient, user_key: &Pubkey, mint: &SplPubkey) -> Result<Option<Instruction>> {
    let token_account = spl_associated_token_account::get_associated_token_address(&sdk_to_spl(user_key), &mint);
    let token_account_info = client.get_account(&spl_to_sdk(&token_account));
    if token_account_info.is_ok() {
        return Ok(None);
    }
    let spl_create_account_ix = spl_associated_token_account::instruction::create_associated_token_account(
        &sdk_to_spl(&user_key),
        &sdk_to_spl(user_key),
        &mint,
        &spl_token::id(),
    );
    let create_account_ix = serde_json::from_value(serde_json::to_value(spl_create_account_ix)?)?;
    Ok(Some(create_account_ix))
}

fn check_airdrop(client: &RpcClient, user_key: &Pubkey, distributor: &Pubkey, program_id: &Pubkey) -> Result<AirdropProof> {

    let base_url = env::var("AIRDROP_BASE_URL")?;
    let url = format!(
        "{}/{}.json",
        base_url,
        user_key
    );

    let response = Client::new().get(&url).send()?;
    if response.status().is_success() {
        let proof: AirdropProof = response.json()?;
        println!("Airdrop found: {:?}", proof);
        let claim_status_key = find_claim_status_key(proof.index, &distributor, program_id);
        println!("Claim status key: {}", claim_status_key );

        let account = client.get_account(&claim_status_key);
        if account.is_ok() {
            return Err(anyhow!("Claim already made."));
        } else {
            println!("{:?} point Airdrop is available to claim.", (proof.amount as f64) / 1_000_000f64);
            return Ok(proof);
        }
    } else if response.status().as_u16() == 404 {
        return Err(anyhow!("No airdrop available."));
    } else {
        return Err(anyhow!("Failed to fetch airdrop info: {}", response.status()));
    }
}

fn find_claim_status_key(index: u64, distributor: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let seeds: &[&[u8]] = &[
        b"ClaimStatus",
        &index.to_le_bytes(),         // index is a u64
        distributor.as_ref(),         // distributor is a &Pubkey
    ];
    Pubkey::find_program_address(seeds, program_id).0
}

fn claim_airdrop(client: &RpcClient, payer: Rc<dyn Signer>, distributor: Pubkey, info: AirdropProof, program_id: &Pubkey) -> Result<()> {

    // Create a token account if needed
    let key = payer.pubkey();

    let mint_string = env::var("MINT_ADDRESS")?;
    let mint = spl_token::solana_program::pubkey::Pubkey::from_str(&mint_string)?;
    let create_token_ix = create_token_account(client, &key, &mint)?;


    let anchor_client = anchor_client::Client::new(Cluster::Mainnet, payer);
    let program = anchor_client.program(merkle_distributor::id());

    let mut builder = program.request();
    if let Some(ix) = create_token_ix {
        builder = builder.instruction(ix);
    }

    let claim_args = merkle_distributor::instruction::Claim {
        amount: info.amount,
        _bump: 0,
        index: info.index,
        proof: info.proof.iter().map(|s| {
            let bytes = hex::decode(s).expect("proof is not a valid hex string");
            let array : [u8 ; 32] = bytes.try_into().expect("proof length is not 32 bytes");
            array
        }).collect(),
    };


    // Get accounts
    let claim_status = find_claim_status_key(info.index, &distributor, program_id);
    let to = spl_associated_token_account::get_associated_token_address(
        &sdk_to_spl(&key),
        &mint);
    let from = spl_associated_token_account::get_associated_token_address(
        &sdk_to_spl(&distributor),
        &mint);

    let claim_accounts = merkle_distributor::accounts::Claim {
        distributor,
        claim_status,
        from: spl_to_sdk(&from),
        to: spl_to_sdk(&to),
        claimant: key,
        payer: key,
        token_program: spl_to_sdk(&spl_token::id()),
        system_program: solana_sdk::system_program::ID,
    };

    let sig = builder.args(claim_args).accounts(claim_accounts).send()?;

    println!("✅ Transaction sent!");
    println!("Signature: {:?}", sig);

    Ok(())
}

fn parse_args(args: &[String]) -> Result<(String, String)> {
    let mut rpc_url = "https://api.mainnet-beta.solana.com".to_string();
    let mut keypair_path = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-u" | "--url" => {
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
  <KEYPAIR_PATH>        Path to the Solana keypair file.

Options:
  -u, --url <RPC_URL>   Override the default RPC URL (default: https://api.mainnet-beta.solana.com)
  -h, --help            Show this help message
"
    );
}
