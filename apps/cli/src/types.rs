use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Deserialize)]
pub struct AuthRequestResponse {
    pub ok: bool,
    pub cooldown_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MeResponse {
    Legacy {
        email: String,
        balance: i64,
        minted: i64,
        sent: i64,
        received: i64,
    },
    Modern {
        email: String,
        balance_base_units: String,
        minted_base_units: String,
        sent_base_units: String,
        received_base_units: String,
        #[serde(default)]
        wrap_allowed: bool,
        #[serde(default)]
        solana_wallet: Option<String>,
        #[serde(default)]
        srpow_supply_owned_base_units: String,
    },
}

impl MeResponse {
    pub fn email(&self) -> &str {
        match self {
            Self::Legacy { email, .. } | Self::Modern { email, .. } => email,
        }
    }

    pub fn balance_display(&self) -> String {
        match self {
            Self::Legacy { balance, .. } => balance.to_string(),
            Self::Modern {
                balance_base_units, ..
            } => format_base_units(balance_base_units),
        }
    }

    pub fn minted_display(&self) -> String {
        match self {
            Self::Legacy { minted, .. } => minted.to_string(),
            Self::Modern {
                minted_base_units, ..
            } => format_base_units(minted_base_units),
        }
    }

    pub fn sent_display(&self) -> String {
        match self {
            Self::Legacy { sent, .. } => sent.to_string(),
            Self::Modern {
                sent_base_units, ..
            } => format_base_units(sent_base_units),
        }
    }

    pub fn received_display(&self) -> String {
        match self {
            Self::Legacy { received, .. } => received.to_string(),
            Self::Modern {
                received_base_units, ..
            } => format_base_units(received_base_units),
        }
    }

    pub fn wrap_allowed(&self) -> Option<bool> {
        match self {
            Self::Legacy { .. } => None,
            Self::Modern { wrap_allowed, .. } => Some(*wrap_allowed),
        }
    }

    pub fn solana_wallet(&self) -> Option<&str> {
        match self {
            Self::Legacy { .. } => None,
            Self::Modern { solana_wallet, .. } => solana_wallet.as_deref(),
        }
    }

    pub fn wrapped_supply_display(&self) -> Option<String> {
        match self {
            Self::Legacy { .. } => None,
            Self::Modern {
                srpow_supply_owned_base_units,
                ..
            } => Some(format_base_units(srpow_supply_owned_base_units)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ChallengeResponse {
    pub challenge_id: String,
    pub nonce_prefix: String,
    pub difficulty_bits: u32,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct MintRequestBody {
    pub challenge_id: String,
    pub solution_nonce: String,
}

#[derive(Debug, Deserialize)]
pub struct MintResponse {
    pub token: TokenSummary,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TokenSummary {
    Legacy {
        id: String,
        value: i64,
        issued_at: String,
    },
    Modern {
        id: String,
        value_base_units: String,
        issued_at: String,
    },
}

impl TokenSummary {
    pub fn id(&self) -> &str {
        match self {
            Self::Legacy { id, .. } | Self::Modern { id, .. } => id,
        }
    }

    pub fn value_display(&self) -> String {
        match self {
            Self::Legacy { value, .. } => value.to_string(),
            Self::Modern {
                value_base_units, ..
            } => format_base_units(value_base_units),
        }
    }

    pub fn issued_at(&self) -> &str {
        match self {
            Self::Legacy { issued_at, .. } | Self::Modern { issued_at, .. } => issued_at,
        }
    }
}

#[derive(Debug)]
pub struct SendRequestBody {
    pub recipient_email: String,
    pub amount: Option<i64>,
    pub amount_base_units: String,
    pub idempotency_key: String,
}

impl SendRequestBody {
    pub fn from_rpow_amount(
        recipient_email: String,
        amount_input: &str,
        idempotency_key: String,
    ) -> anyhow::Result<Self> {
        let amount_base_units = parse_rpow_to_base_units(amount_input)?;
        let legacy_amount = amount_base_units
            .strip_suffix("000000000")
            .and_then(|whole| whole.parse::<i64>().ok());
        Ok(Self {
            recipient_email,
            amount: legacy_amount,
            amount_base_units,
            idempotency_key,
        })
    }
}

impl Serialize for SendRequestBody {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let field_count = if self.amount.is_some() { 4 } else { 3 };
        let mut state = serializer.serialize_struct("SendRequestBody", field_count)?;
        state.serialize_field("recipient_email", &self.recipient_email)?;
        if let Some(amount) = self.amount {
            state.serialize_field("amount", &amount)?;
        }
        state.serialize_field("amount_base_units", &self.amount_base_units)?;
        state.serialize_field("idempotency_key", &self.idempotency_key)?;
        state.end()
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SendResponse {
    Legacy {
        ok: bool,
        transferred: i64,
        recipient_email: String,
        transfer_id: String,
        pending: Option<bool>,
    },
    Modern {
        ok: bool,
        transferred_base_units: String,
        recipient_email: String,
        transfer_id: String,
        pending: Option<bool>,
    },
}

impl SendResponse {
    pub fn ok(&self) -> bool {
        match self {
            Self::Legacy { ok, .. } | Self::Modern { ok, .. } => *ok,
        }
    }

    pub fn transferred_display(&self) -> String {
        match self {
            Self::Legacy { transferred, .. } => transferred.to_string(),
            Self::Modern {
                transferred_base_units,
                ..
            } => format_base_units(transferred_base_units),
        }
    }

    pub fn recipient_email(&self) -> &str {
        match self {
            Self::Legacy {
                recipient_email, ..
            }
            | Self::Modern {
                recipient_email, ..
            } => recipient_email,
        }
    }

    pub fn transfer_id(&self) -> &str {
        match self {
            Self::Legacy { transfer_id, .. } | Self::Modern { transfer_id, .. } => transfer_id,
        }
    }

    pub fn pending(&self) -> bool {
        match self {
            Self::Legacy { pending, .. } | Self::Modern { pending, .. } => {
                pending.unwrap_or(false)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ActivityEntry {
    Legacy {
        r#type: String,
        amount: i64,
        counterparty_email: Option<String>,
        at: String,
    },
    Modern {
        r#type: String,
        amount_base_units: String,
        counterparty_email: Option<String>,
        at: String,
    },
}

impl ActivityEntry {
    pub fn type_name(&self) -> &str {
        match self {
            Self::Legacy { r#type, .. } | Self::Modern { r#type, .. } => r#type,
        }
    }

    pub fn amount_display(&self) -> String {
        match self {
            Self::Legacy { amount, .. } => amount.to_string(),
            Self::Modern {
                amount_base_units, ..
            } => format_base_units(amount_base_units),
        }
    }

    pub fn counterparty_email(&self) -> Option<&str> {
        match self {
            Self::Legacy {
                counterparty_email, ..
            }
            | Self::Modern {
                counterparty_email, ..
            } => counterparty_email.as_deref(),
        }
    }

    pub fn at(&self) -> &str {
        match self {
            Self::Legacy { at, .. } | Self::Modern { at, .. } => at,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum LedgerResponse {
    Legacy {
        total_minted: i64,
        total_transferred: i64,
        circulating_supply: i64,
        current_difficulty_bits: i64,
        user_count: i64,
    },
    Modern {
        total_minted_base_units: String,
        total_transferred_base_units: String,
        circulating_supply_base_units: String,
        minted_supply_counter_base_units: String,
        max_supply_base_units: String,
        current_difficulty_bits: i64,
        current_reward_base_units: String,
        next_reward_base_units: String,
        next_halving_at_base_units: String,
        base_units_to_next_halving: String,
        halving_index: i64,
        is_capped: bool,
        user_count: i64,
    },
}

impl LedgerResponse {
    pub fn total_minted_display(&self) -> String {
        match self {
            Self::Legacy { total_minted, .. } => total_minted.to_string(),
            Self::Modern {
                total_minted_base_units,
                ..
            } => format_base_units(total_minted_base_units),
        }
    }

    pub fn total_transferred_display(&self) -> String {
        match self {
            Self::Legacy {
                total_transferred, ..
            } => total_transferred.to_string(),
            Self::Modern {
                total_transferred_base_units,
                ..
            } => format_base_units(total_transferred_base_units),
        }
    }

    pub fn circulating_supply_display(&self) -> String {
        match self {
            Self::Legacy {
                circulating_supply,
                ..
            } => circulating_supply.to_string(),
            Self::Modern {
                circulating_supply_base_units,
                ..
            } => format_base_units(circulating_supply_base_units),
        }
    }

    pub fn current_difficulty_bits(&self) -> i64 {
        match self {
            Self::Legacy {
                current_difficulty_bits,
                ..
            }
            | Self::Modern {
                current_difficulty_bits,
                ..
            } => *current_difficulty_bits,
        }
    }

    pub fn user_count(&self) -> i64 {
        match self {
            Self::Legacy { user_count, .. } | Self::Modern { user_count, .. } => *user_count,
        }
    }

    pub fn current_reward_display(&self) -> Option<String> {
        match self {
            Self::Legacy { .. } => None,
            Self::Modern {
                current_reward_base_units,
                ..
            } => Some(format_base_units(current_reward_base_units)),
        }
    }

    pub fn next_reward_display(&self) -> Option<String> {
        match self {
            Self::Legacy { .. } => None,
            Self::Modern {
                next_reward_base_units,
                ..
            } => Some(format_base_units(next_reward_base_units)),
        }
    }

    pub fn next_halving_at_display(&self) -> Option<String> {
        match self {
            Self::Legacy { .. } => None,
            Self::Modern {
                next_halving_at_base_units,
                ..
            } => Some(format_base_units(next_halving_at_base_units)),
        }
    }

    pub fn units_to_next_halving_display(&self) -> Option<String> {
        match self {
            Self::Legacy { .. } => None,
            Self::Modern {
                base_units_to_next_halving,
                ..
            } => Some(format_base_units(base_units_to_next_halving)),
        }
    }

    pub fn halving_index(&self) -> Option<i64> {
        match self {
            Self::Legacy { .. } => None,
            Self::Modern { halving_index, .. } => Some(*halving_index),
        }
    }

    pub fn is_capped(&self) -> Option<bool> {
        match self {
            Self::Legacy { .. } => None,
            Self::Modern { is_capped, .. } => Some(*is_capped),
        }
    }

    pub fn max_supply_display(&self) -> Option<String> {
        match self {
            Self::Legacy { .. } => None,
            Self::Modern {
                max_supply_base_units,
                ..
            } => Some(format_base_units(max_supply_base_units)),
        }
    }

    pub fn minted_counter_display(&self) -> Option<String> {
        match self {
            Self::Legacy { .. } => None,
            Self::Modern {
                minted_supply_counter_base_units,
                ..
            } => Some(format_base_units(minted_supply_counter_base_units)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    pub retry_after: Option<u64>,
}

const BASE_UNITS_PER_RPOW: i128 = 1_000_000_000;

fn format_base_units(raw: &str) -> String {
    let Ok(value) = raw.parse::<i128>() else {
        return raw.to_string();
    };
    let whole = value / BASE_UNITS_PER_RPOW;
    let frac = value % BASE_UNITS_PER_RPOW;
    if frac == 0 {
        return whole.to_string();
    }
    let frac_text = frac
        .to_string()
        .trim_start_matches('-')
        .to_string()
        .chars()
        .collect::<String>();
    let frac_text = format!("{:0>9}", frac_text).trim_end_matches('0').to_string();
    format!("{whole}.{frac_text}")
}

fn parse_rpow_to_base_units(raw: &str) -> anyhow::Result<String> {
    let input = raw.trim();
    if input.is_empty() {
        anyhow::bail!("amount must not be empty");
    }
    if input.starts_with('-') {
        anyhow::bail!("amount must be positive");
    }
    let parts: Vec<_> = input.split('.').collect();
    if parts.len() > 2 {
        anyhow::bail!("amount must be a decimal with at most one dot");
    }
    let whole = parts[0];
    let frac = parts.get(1).copied().unwrap_or("");
    if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
        anyhow::bail!("amount must contain only digits");
    }
    if !frac.chars().all(|c| c.is_ascii_digit()) {
        anyhow::bail!("amount must contain only digits");
    }
    if frac.len() > 9 {
        anyhow::bail!("amount supports at most 9 decimal places");
    }
    let whole_value = whole
        .parse::<i128>()
        .map_err(|_| anyhow::anyhow!("amount is too large"))?;
    let frac_padded = format!("{:0<9}", frac);
    let frac_value = frac_padded
        .parse::<i128>()
        .map_err(|_| anyhow::anyhow!("amount is too large"))?;
    let total = whole_value
        .checked_mul(BASE_UNITS_PER_RPOW)
        .and_then(|v| v.checked_add(frac_value))
        .ok_or_else(|| anyhow::anyhow!("amount is too large"))?;
    if total <= 0 {
        anyhow::bail!("amount must be positive");
    }
    Ok(total.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_legacy_me_response() {
        let me: MeResponse = serde_json::from_str(
            r#"{"email":"a@b.com","balance":2,"minted":3,"sent":1,"received":0}"#,
        )
        .unwrap();
        assert_eq!(me.balance_display(), "2");
    }

    #[test]
    fn deserializes_modern_me_response() {
        let me: MeResponse = serde_json::from_str(
            r#"{"email":"a@b.com","balance_base_units":"2000000000","minted_base_units":"3000000000","sent_base_units":"1000000000","received_base_units":"0","wrap_allowed":false,"solana_wallet":null,"srpow_supply_owned_base_units":"0"}"#,
        )
        .unwrap();
        assert_eq!(me.balance_display(), "2");
    }

    #[test]
    fn serializes_send_request_for_both_protocols() {
        let body = SendRequestBody::from_rpow_amount(
            "b@c.com".to_string(),
            "2",
            "idem-12345678".to_string(),
        )
        .unwrap();
        let json = serde_json::to_value(body).unwrap();
        assert_eq!(json["amount"], 2);
        assert_eq!(json["amount_base_units"], "2000000000");
    }

    #[test]
    fn serializes_decimal_send_request_for_modern_protocol() {
        let body = SendRequestBody::from_rpow_amount(
            "b@c.com".to_string(),
            "1.25",
            "idem-12345678".to_string(),
        )
        .unwrap();
        let json = serde_json::to_value(body).unwrap();
        assert_eq!(json.get("amount"), None);
        assert_eq!(json["amount_base_units"], "1250000000");
    }

    #[test]
    fn parses_fractional_rpow_amounts() {
        assert_eq!(parse_rpow_to_base_units("0.000000001").unwrap(), "1");
        assert_eq!(parse_rpow_to_base_units("1.25").unwrap(), "1250000000");
    }

    #[test]
    fn rejects_overprecise_rpow_amounts() {
        assert!(parse_rpow_to_base_units("0.0000000001").is_err());
    }
}
