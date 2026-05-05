/// EIP-712 order signing for Polymarket CTF Exchange via onchainos.
///
/// V1: legacy exchange (0x4bFb41...), domain version "1", struct has taker/nonce/feeRateBps.
/// V2: new exchange (0xE11118...), domain version "2", struct has timestamp/metadata/builder.
///
/// All signing is delegated to `onchainos wallet sign-message --type eip712`.
/// No local private key is used or stored by this module.
use anyhow::Result;

// ─── V1 ───────────────────────────────────────────────────────────────────────

/// Parameters for a Polymarket V1 limit order.
pub struct OrderParams {
    pub salt: u64,
    pub maker: String,
    pub signer: String,
    pub taker: String,
    pub token_id: String,
    pub maker_amount: u64,
    pub taker_amount: u64,
    pub expiration: u64,
    pub nonce: u64,
    pub fee_rate_bps: u64,
    pub side: u8,           // 0=BUY, 1=SELL
    pub signature_type: u8, // 0=EOA, 1=Proxy
}

/// Sign a V1 Polymarket order via `onchainos sign-message --type eip712`.
pub async fn sign_order_via_onchainos(order: &OrderParams, neg_risk: bool) -> Result<String> {
    use crate::config::Contracts;
    let verifying_contract = Contracts::exchange_for(neg_risk);

    let json = serde_json::to_string(&serde_json::json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"}
            ],
            "Order": [
                {"name": "salt", "type": "uint256"},
                {"name": "maker", "type": "address"},
                {"name": "signer", "type": "address"},
                {"name": "taker", "type": "address"},
                {"name": "tokenId", "type": "uint256"},
                {"name": "makerAmount", "type": "uint256"},
                {"name": "takerAmount", "type": "uint256"},
                {"name": "expiration", "type": "uint256"},
                {"name": "nonce", "type": "uint256"},
                {"name": "feeRateBps", "type": "uint256"},
                {"name": "side", "type": "uint8"},
                {"name": "signatureType", "type": "uint8"}
            ]
        },
        "primaryType": "Order",
        "domain": {
            "name": "Polymarket CTF Exchange",
            "version": "1",
            "chainId": 137,
            "verifyingContract": verifying_contract
        },
        "message": {
            "salt": order.salt.to_string(),
            "maker": order.maker,
            "signer": order.signer,
            "taker": order.taker,
            "tokenId": order.token_id,
            "makerAmount": order.maker_amount.to_string(),
            "takerAmount": order.taker_amount.to_string(),
            "expiration": order.expiration.to_string(),
            "nonce": order.nonce.to_string(),
            "feeRateBps": order.fee_rate_bps.to_string(),
            "side": order.side,
            "signatureType": order.signature_type
        }
    }))
    .expect("V1 Order EIP-712 JSON serialization failed");

    crate::onchainos::sign_eip712(&json).await
}

// ─── V2 ───────────────────────────────────────────────────────────────────────

/// Parameters for a Polymarket V2 limit order.
///
/// Key differences from V1:
///   - No `taker`, `nonce`, or `feeRateBps` (fees are now protocol-enforced)
///   - `expiration` is NOT in the signed struct (it goes in the outer API request wrapper)
///   - `timestamp_ms`: millisecond Unix timestamp added to the signed struct
///   - `metadata`: bytes32 optional metadata (zero for standard orders)
///   - `builder`: bytes32 builder code for fee attribution (zero for non-builders)
pub struct OrderParamsV2 {
    pub salt: u64,
    pub maker: String,
    pub signer: String,
    pub token_id: String,
    pub maker_amount: u64,
    pub taker_amount: u64,
    pub side: u8,           // 0=BUY, 1=SELL
    pub signature_type: u8, // 0=EOA, 1=Proxy, 2=GnosisSafe, 3=POLY_1271
    pub timestamp_ms: u64,  // millisecond Unix timestamp
    pub metadata: String,   // bytes32 hex: "0x000...000" for standard orders
    pub builder: String,    // bytes32 hex: "0x000...000" for non-builders
}

/// Sign a V2 Polymarket order via `onchainos sign-message --type eip712`.
///
/// Uses domain version "2" and the new V2 exchange contract address.
/// `expiration` is not part of the signed struct in V2 — pass it separately in the API body.
pub async fn sign_order_v2_via_onchainos(order: &OrderParamsV2, neg_risk: bool) -> Result<String> {
    use crate::config::Contracts;
    let verifying_contract = Contracts::exchange_for_v2(neg_risk);

    let json = serde_json::to_string(&serde_json::json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"}
            ],
            "Order": [
                {"name": "salt", "type": "uint256"},
                {"name": "maker", "type": "address"},
                {"name": "signer", "type": "address"},
                {"name": "tokenId", "type": "uint256"},
                {"name": "makerAmount", "type": "uint256"},
                {"name": "takerAmount", "type": "uint256"},
                {"name": "side", "type": "uint8"},
                {"name": "signatureType", "type": "uint8"},
                {"name": "timestamp", "type": "uint256"},
                {"name": "metadata", "type": "bytes32"},
                {"name": "builder", "type": "bytes32"}
            ]
        },
        "primaryType": "Order",
        "domain": {
            "name": "Polymarket CTF Exchange",
            "version": "2",
            "chainId": 137,
            "verifyingContract": verifying_contract
        },
        "message": {
            "salt": order.salt.to_string(),
            "maker": order.maker,
            "signer": order.signer,
            "tokenId": order.token_id,
            "makerAmount": order.maker_amount.to_string(),
            "takerAmount": order.taker_amount.to_string(),
            "side": order.side,
            "signatureType": order.signature_type,
            "timestamp": order.timestamp_ms.to_string(),
            "metadata": order.metadata,
            "builder": order.builder
        }
    }))
    .expect("V2 Order EIP-712 JSON serialization failed");

    crate::onchainos::sign_eip712(&json).await
}

/// Bytes32 zero value — used as default metadata and builder in V2 orders.
pub const BYTES32_ZERO: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";

// ─── Deposit Wallet (sig_type=3 / POLY_1271) ─────────────────────────────────

/// A single call in a DepositWallet batch.
pub struct WalletCall {
    pub target: String, // contract to call
    pub value: u64,
    pub data: String, // hex-encoded calldata
}

/// Parameters for signing a DepositWallet batch transaction.
///
/// The batch is submitted via the Polymarket relayer (POST /submit, type=WALLET).
/// The owner EOA signs an EIP-712 `Batch` struct; the deposit wallet contract
/// validates via ERC-1271 (`isValidSignature`).
///
/// Domain: name="DepositWallet", version="1", chainId=137, verifyingContract=<wallet_addr>
pub struct BatchParams {
    pub wallet: String,   // deposit wallet address (verifying contract)
    pub nonce: u64,
    pub deadline: u64,    // Unix timestamp
    pub calls: Vec<WalletCall>,
}

/// Sign a DepositWallet batch via `onchainos wallet sign-message --type eip712`.
pub async fn sign_batch_via_onchainos(params: &BatchParams) -> Result<String> {
    let calls_json: Vec<serde_json::Value> = params.calls.iter().map(|c| {
        serde_json::json!({
            "target": c.target,
            "value":  c.value.to_string(),
            "data":   c.data,
        })
    }).collect();

    let json = serde_json::to_string(&serde_json::json!({
        "types": {
            "EIP712Domain": [
                {"name": "name",             "type": "string"},
                {"name": "version",          "type": "string"},
                {"name": "chainId",          "type": "uint256"},
                {"name": "verifyingContract","type": "address"}
            ],
            "Call": [
                {"name": "target", "type": "address"},
                {"name": "value",  "type": "uint256"},
                {"name": "data",   "type": "bytes"}
            ],
            "Batch": [
                {"name": "wallet",   "type": "address"},
                {"name": "nonce",    "type": "uint256"},
                {"name": "deadline", "type": "uint256"},
                {"name": "calls",    "type": "Call[]"}
            ]
        },
        "primaryType": "Batch",
        "domain": {
            "name": "DepositWallet",
            "version": "1",
            "chainId": 137,
            "verifyingContract": params.wallet,
        },
        "message": {
            "wallet":   params.wallet,
            "nonce":    params.nonce.to_string(),
            "deadline": params.deadline.to_string(),
            "calls":    calls_json,
        }
    }))
    .expect("Batch EIP-712 JSON serialization failed");

    crate::onchainos::sign_eip712(&json).await
}

/// Sign a V2 order for a deposit wallet (POLY_1271 / ERC-1271, signature_type=3).
///
/// The order struct is identical to the standard V2 order, but:
/// - `maker` and `signer` are both set to the deposit wallet address (not the EOA)
/// - `signature_type` = 3 (POLY_1271)
/// - The resulting ECDSA signature is validated on-chain via the deposit wallet's
///   `isValidSignature` (ERC-1271). The CLOB passes the order hash and signature
///   to the wallet contract for verification.
///
/// Note: Full ERC-7739 (TypedDataSign) wrapping may be required for replay protection.
/// The current implementation produces a standard 65-byte EIP-712 signature over the
/// V2 Order struct with the exchange as verifying contract. Verify against the live
/// Polymarket CLOB with deposit wallets — if rejected, the wrapping layer needs to be
/// added here using the TypedDataSign envelope (EIP-7739).
pub async fn sign_order_v2_poly1271_via_onchainos(order: &OrderParamsV2, neg_risk: bool) -> Result<String> {
    // The signing payload is identical to the standard V2 order — only signature_type differs.
    // POLY_1271 validation is handled by the CLOB calling isValidSignature on the deposit wallet.
    sign_order_v2_via_onchainos(order, neg_risk).await
}
