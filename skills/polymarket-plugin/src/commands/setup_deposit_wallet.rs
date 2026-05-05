/// `polymarket setup-deposit-wallet` — deploy a Polymarket deposit wallet and configure
/// POLY_1271 trading mode. One-time relayer-funded setup; all subsequent trading is gasless.
///
/// Flow (6 steps):
///   1. Identify owner EOA from onchainos.
///   2. Check for existing deposit wallet on-chain (idempotent — skips deploy if found).
///   3. Deploy via relayer WALLET-CREATE.
///   4. Submit pUSD + CTF ERC-1155 approvals as a signed WALLET batch.
///   5. Sync CLOB balance-allowance endpoint with signature_type=3.
///   6. Save deposit_wallet address and switch mode to DepositWallet in creds.json.
///
/// Existing EOA / PolyProxy users are NOT affected — this command is only needed once
/// by new users. Existing users' creds.json already has a mode set and this command
/// is never triggered automatically.
use anyhow::{bail, Result};
use reqwest::Client;

use crate::api::{get_wallet_nonce, relayer_wallet_batch, relayer_wallet_create, sync_balance_allowance_deposit_wallet};
use crate::auth::ensure_credentials;
use crate::config::{Contracts, TradingMode};
use crate::onchainos::get_wallet_address;
use crate::signing::{sign_batch_via_onchainos, BatchParams, WalletCall};

pub async fn run(dry_run: bool) -> Result<()> {
    match run_inner(dry_run).await {
        Ok(()) => Ok(()),
        Err(e) => { println!("{}", super::error_response(&e, Some("setup-deposit-wallet"), None)); Ok(()) }
    }
}

async fn run_inner(dry_run: bool) -> Result<()> {
    let client = Client::new();
    let owner_addr = get_wallet_address().await?;

    eprintln!("[polymarket] Setting up deposit wallet for EOA: {}", owner_addr);

    // ── Step 1: check for existing deposit wallet ─────────────────────────────
    let existing = crate::onchainos::get_existing_deposit_wallet(&owner_addr).await;

    if let Some(ref addr) = existing {
        eprintln!("[polymarket] Existing deposit wallet found: {}", addr);
    }

    if dry_run {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "dry_run": true,
                "data": {
                    "owner": owner_addr,
                    "existing_wallet": existing,
                    "factory": Contracts::DEPOSIT_WALLET_FACTORY,
                    "note": if existing.is_some() {
                        "dry-run: deposit wallet already deployed — would skip deploy, re-run approvals + balance sync"
                    } else {
                        "dry-run: would deploy via relayer WALLET-CREATE, then approve pUSD + CTF, then sync CLOB"
                    }
                }
            }))?
        );
        return Ok(());
    }

    // ── Step 2: deploy wallet if not already deployed ─────────────────────────
    let wallet_addr = if let Some(addr) = existing {
        eprintln!("[polymarket] Skipping deploy — deposit wallet already exists.");
        addr
    } else {
        eprintln!("[polymarket] Deploying deposit wallet...");

        // Gasless deployment via the Polymarket relayer (WALLET-CREATE, no signature required).
        // The relayer pays gas and deploys a deterministic ERC-1967 proxy.
        // Note: requires Polymarket builder authorization — regular EOAs cannot call the
        // factory directly due to OnlyOperator access control.
        match relayer_wallet_create(&client, &owner_addr).await {
            Ok(addr) => {
                eprintln!("[polymarket] Deposit wallet deployed: {}", addr);
                addr
            }
            Err(relayer_err) => {
                // Compute predicted address so the user can receive funds now and deploy later
                let predicted = crate::onchainos::predict_deposit_wallet_address(&owner_addr).await;
                bail!(
                    "Deposit wallet deployment failed: {}. \
                     The factory requires Polymarket builder authorization (OnlyOperator). \
                     To deploy your wallet, visit the Polymarket app (app.polymarket.com), \
                     connect this wallet ({}), and complete the setup flow. \
                     Then re-run `polymarket-plugin quickstart` to continue.{}",
                    relayer_err,
                    &owner_addr[..std::cmp::min(10, owner_addr.len())],
                    predicted
                        .map(|p| format!(" Your predicted wallet address: {}", p))
                        .unwrap_or_default()
                );
            }
        }
    };

    // ── Step 3: build approval batch calls ───────────────────────────────────
    // Approvals needed for POLY_1271 (sig_type=3) trading:
    //   a. pUSD → CTF Exchange V2 (normal markets)
    //   b. pUSD → Neg Risk CTF Exchange V2 (neg_risk markets)
    //   c. CTF ERC-1155 setApprovalForAll → CTF Exchange V2 (sell / redeem)
    //   d. CTF ERC-1155 setApprovalForAll → Neg Risk CTF Exchange V2 (sell neg_risk)
    //   e. CTF ERC-1155 setApprovalForAll → Neg Risk Adapter (redeem neg_risk)
    let calls = build_approval_calls();

    eprintln!("[polymarket] Submitting {} approval calls via WALLET batch...", calls.len());

    // ── Step 4: fetch nonce and sign batch ────────────────────────────────────
    let nonce = get_wallet_nonce(&client, &owner_addr).await
        .map_err(|e| anyhow::anyhow!("Could not fetch wallet nonce from relayer: {}", e))?;

    let deadline = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() + 300; // 5-minute window

    let calls_json: Vec<serde_json::Value> = calls.iter().map(|c| serde_json::json!({
        "target": c.target,
        "value":  c.value.to_string(),
        "data":   c.data,
    })).collect();

    let batch_params = BatchParams {
        wallet: wallet_addr.clone(),
        nonce,
        deadline,
        calls,
    };

    let batch_sig = sign_batch_via_onchainos(&batch_params).await
        .map_err(|e| anyhow::anyhow!("Batch signing failed: {}", e))?;

    let batch_tx = relayer_wallet_batch(
        &client,
        &owner_addr,
        &wallet_addr,
        nonce,
        deadline,
        calls_json,
        &batch_sig,
    ).await?;

    eprintln!("[polymarket] Approval batch submitted: {}", batch_tx);
    eprintln!("[polymarket] Waiting for approval batch to confirm...");
    crate::onchainos::wait_for_tx_receipt(&batch_tx, 120).await?;
    eprintln!("[polymarket] Approvals confirmed.");

    // ── Step 5: sync CLOB balance-allowance ──────────────────────────────────
    eprintln!("[polymarket] Syncing CLOB balance-allowance (signature_type=3)...");
    let creds = ensure_credentials(&client, &owner_addr).await?;
    sync_balance_allowance_deposit_wallet(&client, &wallet_addr, &owner_addr, &creds).await
        .unwrap_or_else(|e| eprintln!("[polymarket] Warning: balance sync failed ({}); retry with `polymarket balance`.", e));

    // ── Step 6: save mode + wallet address ───────────────────────────────────
    let mut updated_creds = creds;
    updated_creds.deposit_wallet = Some(wallet_addr.clone());
    updated_creds.mode = TradingMode::DepositWallet;
    crate::config::save_credentials(&updated_creds)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "data": {
                "owner":          owner_addr,
                "deposit_wallet": wallet_addr,
                "mode":           "deposit_wallet",
                "approval_tx":    batch_tx,
                "note": "Deposit wallet ready. Fund it with pUSD and trade with signature_type=3 (POLY_1271). All trading is gasless."
            }
        }))?
    );
    Ok(())
}

/// Build the 5 approval calls required for full deposit wallet trading capability.
fn build_approval_calls() -> Vec<WalletCall> {
    use sha3::{Digest, Keccak256};

    // ERC-20 approve(address spender, uint256 amount) — approve max uint256
    let approve_selector = hex::encode(&Keccak256::digest(b"approve(address,uint256)")[..4]);
    // ERC-1155 setApprovalForAll(address operator, bool approved)
    let set_approval_selector = hex::encode(&Keccak256::digest(b"setApprovalForAll(address,bool)")[..4]);

    let max_uint = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

    let make_erc20_approve = |token: &str, spender: &str| -> WalletCall {
        WalletCall {
            target: token.to_string(),
            value: 0,
            data: format!("0x{}{}{}", approve_selector, pad_address_hex(spender), max_uint),
        }
    };

    let make_set_approval = |operator: &str| -> WalletCall {
        WalletCall {
            target: Contracts::CTF.to_string(),
            value: 0,
            data: format!(
                "0x{}{}0000000000000000000000000000000000000000000000000000000000000001",
                set_approval_selector,
                pad_address_hex(operator)
            ),
        }
    };

    vec![
        // a. pUSD → CTF Exchange V2
        make_erc20_approve(Contracts::PUSD, Contracts::CTF_EXCHANGE_V2),
        // b. pUSD → Neg Risk CTF Exchange V2
        make_erc20_approve(Contracts::PUSD, Contracts::NEG_RISK_CTF_EXCHANGE_V2),
        // c. CTF → CTF Exchange V2 (sell / redeem)
        make_set_approval(Contracts::CTF_EXCHANGE_V2),
        // d. CTF → Neg Risk CTF Exchange V2
        make_set_approval(Contracts::NEG_RISK_CTF_EXCHANGE_V2),
        // e. CTF → Neg Risk Adapter (redeem neg_risk)
        make_set_approval(Contracts::NEG_RISK_ADAPTER),
    ]
}

/// Pad an address to a 32-byte ABI word (left-padded with zeros, no 0x prefix).
fn pad_address_hex(addr: &str) -> String {
    let stripped = addr.trim_start_matches("0x");
    format!("{:0>64}", stripped)
}

