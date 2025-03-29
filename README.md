# Merkle Distributor Claim CLI

CLI tool for claiming merkle distributions with a Solana keypair. Compatible with [this version](https://github.com/saber-hq/merkle-distributor)
of the distributor.

## Using the claim tool

* Clone this repository to your local machine.
* Ensure that rust is installed on your machine. You can install it by following the instructions [here](https://www.rust-lang.org/tools/install).
* Run the following command to claim your distribution:

```bash
cargo run -- <PATH_TO_KEYPAIR_RECEIVING_THE_CLAIM> [--url <RPC_URL>] 
```

The tool will work for different claims by setting the
following environment variables:
    
```bash
AIRDROP_BASE_URL=https://solanavault.github.io/simd-228-vpts-airdrop/proofs
DISTRIBUTOR_ADDRESS=BAzvEJH5w7igbbkiRLhCD89b1gLtrZz6B9wxxuh3ocJz
MINT_ADDRESS=vPtS4ywrbEuufwPkBXsCYkeTBfpzCd6hF52p8kJGt9b
```