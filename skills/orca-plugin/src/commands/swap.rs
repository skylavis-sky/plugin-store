use crate::api;
use crate::config::{
    DEFAULT_SLIPPAGE_BPS, PRICE_IMPACT_BLOCK_THRESHOLD, PRICE_IMPACT_WARN_THRESHOLD,
    SOL_NATIVE_MINT, WSOL_MINT,
};
use crate::onchainos;
use clap::Args;
use serde::Serialize;
use std::process::Command;

#[derive(Args, Debug)]
pub struct SwapArgs {
    /// Input token mint address (use native SOL: 11111111111111111111111111111111)
    #[arg(long)]
    pub from_token: String,

    /// Output token mint address
    #[arg(long)]
    pub to_token: String,

    /// Amount in human-readable units (e.g. 0.5 for 0.5 SOL, 10 for 10 USDC)
    #[arg(long)]
    pub amount: f64,

    /// Slippage tolerance in basis points (default: 50 = 0.5%)
    #[arg(long, default_value_t = DEFAULT_SLIPPAGE_BPS)]
    pub slippage_bps: u64,

    /// Skip security scan of output token (not recommended)
    #[arg(long)]
    pub skip_security_check: bool,

    /// Confirm execution — required to execute on-chain. Without this flag, shows a preview.
    #[arg(long)]
    pub confirm: bool,
}

#[derive(Serialize)]
struct SwapOutput {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    dry_run: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tx_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    solscan_url: Option<String>,
    from_token: String,
    to_token: String,
    amount: f64,
    slippage_bps: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    estimated_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    estimated_price_impact_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fee_rate_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pool_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

pub async fn execute(args: &SwapArgs, dry_run: bool) -> anyhow::Result<()> {
    // ─── Security scan of output token ───
    if !args.skip_security_check {
        let to_check = if args.to_token == SOL_NATIVE_MINT {
            WSOL_MINT
        } else {
            &args.to_token
        };
        match onchainos::security_token_scan(to_check) {
            Ok(risk) if risk == "block" => {
                let output = SwapOutput {
                    ok: false,
                    dry_run: None,
                    tx_hash: None,
                    solscan_url: None,
                    from_token: args.from_token.clone(),
                    to_token: args.to_token.clone(),
                    amount: args.amount,
                    slippage_bps: args.slippage_bps,
                    estimated_output: None,
                    minimum_output: None,
                    estimated_price_impact_pct: None,
                    fee_rate_pct: None,
                    warning: None,
                    error: Some(format!(
                        "Security scan blocked token {}: high-risk token, swap aborted",
                        args.to_token
                    )),
                    pool_address: None,
                    note: None,
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
                return Ok(());
            }
            Ok(risk) if risk == "warn" => {
                eprintln!(
                    "WARNING: Security scan returned 'warn' for token {}. Proceeding with caution.",
                    args.to_token
                );
            }
            _ => {}
        }
    }

    // ─── Fetch pool info for price impact estimation (used by all paths) ───
    let client = reqwest::Client::new();
    let all_pools = api::fetch_all_pools(&client).await?;

    let normalize = |mint: &str| -> String {
        if mint == SOL_NATIVE_MINT {
            WSOL_MINT.to_string()
        } else {
            mint.to_string()
        }
    };
    let from_norm = normalize(&args.from_token);
    let to_norm = normalize(&args.to_token);

    let mut matching = api::filter_pools_by_pair(&all_pools, &from_norm, &to_norm);
    matching.sort_by(|a, b| {
        b.tvl
            .unwrap_or(0.0)
            .partial_cmp(&a.tvl.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let (best_pool_address, price_impact, pool_warning, fee_rate_pct_opt, estimated_out, minimum_out) =
        if let Some(pool) = matching.first() {
            let price = pool.price.unwrap_or(0.0);
            let tvl = pool.tvl.unwrap_or(1_000_000.0);
            let impact = api::estimate_price_impact(args.amount * price.max(1.0), tvl);
            let warn = if impact >= PRICE_IMPACT_WARN_THRESHOLD {
                Some(format!("Estimated price impact {:.2}%", impact))
            } else {
                None
            };
            let fee_pct = pool.lp_fee_rate.map(|r| r * 100.0);
            // estimated_output in output token units (approximate)
            let est_out = if price > 0.0 {
                Some(format!("{:.6}", args.amount / price))
            } else {
                None
            };
            // minimum_output accounting for slippage
            let min_out = if let Some(ref est) = est_out {
                est.parse::<f64>().ok().map(|v| {
                    format!("{:.6}", v * (1.0 - args.slippage_bps as f64 / 10_000.0))
                })
            } else {
                None
            };
            (Some(pool.address.clone()), impact, warn, fee_pct, est_out, min_out)
        } else {
            eprintln!(
                "No pool found for pair {} / {} — proceeding with swap anyway (onchainos will route)",
                args.from_token, args.to_token
            );
            (None, 0.0, None, None, None, None)
        };

    // ─── Block if price impact is too high ───
    if price_impact >= PRICE_IMPACT_BLOCK_THRESHOLD {
        let output = SwapOutput {
            ok: false,
            dry_run: None,
            tx_hash: None,
            solscan_url: None,
            from_token: args.from_token.clone(),
            to_token: args.to_token.clone(),
            amount: args.amount,
            slippage_bps: args.slippage_bps,
            estimated_output: estimated_out,
            minimum_output: minimum_out,
            estimated_price_impact_pct: Some(price_impact),
            fee_rate_pct: fee_rate_pct_opt,
            warning: None,
            error: Some(format!(
                "Price impact {:.2}% exceeds block threshold of {}%. Swap aborted.",
                price_impact, PRICE_IMPACT_BLOCK_THRESHOLD
            )),
            pool_address: best_pool_address,
            note: None,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    // ─── dry_run or confirm gate — show enriched preview ───
    if dry_run || !args.confirm {
        let output = SwapOutput {
            ok: true,
            dry_run: Some(dry_run),
            tx_hash: None,
            solscan_url: None,
            from_token: args.from_token.clone(),
            to_token: args.to_token.clone(),
            amount: args.amount,
            slippage_bps: args.slippage_bps,
            estimated_output: estimated_out,
            minimum_output: minimum_out,
            estimated_price_impact_pct: Some(price_impact),
            fee_rate_pct: fee_rate_pct_opt,
            warning: pool_warning,
            error: None,
            pool_address: best_pool_address,
            note: Some("Re-run with --confirm to execute on-chain.".to_string()),
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    // ─── Execute swap via onchainos dex swap execute ───
    // Convert slippage from bps to percentage string
    let slippage_pct = format!("{:.4}", args.slippage_bps as f64 / 100.0);

    let result = execute_swap_onchainos(
        &args.from_token,
        &args.to_token,
        args.amount,
        &slippage_pct,
    )
    .await?;

    let tx_hash = onchainos::extract_tx_hash(&result);
    let solscan_url = if !tx_hash.is_empty() && tx_hash != "pending" {
        Some(format!("https://solscan.io/tx/{}", tx_hash))
    } else {
        None
    };

    let output = SwapOutput {
        ok: result["ok"].as_bool().unwrap_or(false),
        dry_run: None,
        tx_hash: Some(tx_hash),
        solscan_url,
        from_token: args.from_token.clone(),
        to_token: args.to_token.clone(),
        amount: args.amount,
        slippage_bps: args.slippage_bps,
        estimated_output: estimated_out,
        minimum_output: minimum_out,
        estimated_price_impact_pct: Some(price_impact),
        fee_rate_pct: fee_rate_pct_opt,
        warning: pool_warning,
        error: if result["ok"].as_bool().unwrap_or(false) {
            None
        } else {
            result["error"]
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| result["message"].as_str().map(|s| s.to_string()))
        },
        pool_address: best_pool_address,
        note: None,
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// Execute swap via `onchainos swap execute`.
/// This is the primary path — onchainos handles routing, signing, and broadcasting.
async fn execute_swap_onchainos(
    from_token: &str,
    to_token: &str,
    amount: f64,
    slippage_pct: &str,
) -> anyhow::Result<serde_json::Value> {
    // Resolve wallet address
    let wallet = crate::onchainos::resolve_wallet_solana()?;
    let amount_str = amount.to_string();
    let output = Command::new("onchainos")
        .args([
            "swap",
            "execute",
            "--chain",
            "501",
            "--from",
            from_token,
            "--to",
            to_token,
            "--readable-amount",
            &amount_str,
            "--slippage",
            slippage_pct,
            "--wallet",
            &wallet,
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stdout.trim().is_empty() {
        anyhow::bail!(
            "onchainos swap execute returned empty output. stderr: {}",
            stderr
        );
    }

    serde_json::from_str(&stdout).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse onchainos output: {}. stdout: {}",
            e,
            stdout
        )
    })
}
