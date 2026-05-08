mod api;
mod pow;
mod session;
mod types;

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use uuid::Uuid;

use crate::api::ApiClient;
use crate::pow::verify_solution;
use crate::session::{clear_session, load_session, save_session, SessionState};
use crate::types::{MintRequestBody, SendRequestBody};

const DEFAULT_BASE_URL: &str = "http://localhost:8080";

#[derive(Debug, Parser)]
#[command(name = "rpow", version, about = "Rust CLI client for the RPOW server")]
struct Cli {
    #[arg(long, global = true, env = "RPOW_BASE_URL", default_value = DEFAULT_BASE_URL)]
    base_url: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Login(LoginArgs),
    CookieLogin(CookieLoginArgs),
    Logout,
    Me,
    Mine(MineArgs),
    Send(SendArgs),
    Activity,
    Ledger,
}

#[derive(Debug, Args)]
struct LoginArgs {
    #[arg(long)]
    email: String,
}

#[derive(Debug, Args)]
struct CookieLoginArgs {
    #[arg(long)]
    cookie: Option<String>,
}

#[derive(Debug, Args)]
struct MineArgs {
    #[arg(long)]
    once: bool,
}

#[derive(Debug, Args)]
struct SendArgs {
    #[arg(long)]
    to: String,
    #[arg(long)]
    amount: i64,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Login(args) => login(&cli.base_url, args),
        Commands::CookieLogin(args) => cookie_login(&cli.base_url, args),
        Commands::Logout => logout(),
        Commands::Me => me(),
        Commands::Mine(args) => mine(args),
        Commands::Send(args) => send(args),
        Commands::Activity => activity(),
        Commands::Ledger => ledger(&cli.base_url),
    }
}

fn login(base_url: &str, args: LoginArgs) -> Result<()> {
    let client = ApiClient::new(base_url.to_string(), None)?;
    let response = client.auth_request(&args.email)?;
    if !response.ok {
        anyhow::bail!("server did not accept auth request");
    }

    println!("magic link requested for {}", args.email);
    println!("cooldown: {}s", response.cooldown_seconds);
    println!("paste the full magic link URL from your email:");
    print!("> ");
    io::stdout().flush().ok();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read magic link from stdin")?;
    let session_cookie = client.verify_magic_link(input.trim())?;
    save_session(&SessionState {
        base_url: client.base_url().to_string(),
        session_cookie,
    })?;
    println!("login complete");
    Ok(())
}

fn cookie_login(base_url: &str, args: CookieLoginArgs) -> Result<()> {
    let cookie_input = match args.cookie {
        Some(value) => value,
        None => {
            println!("paste the rpow_session cookie value or a full `rpow_session=...` string:");
            print!("> ");
            io::stdout().flush().ok();
            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .context("failed to read cookie from stdin")?;
            input.trim().to_string()
        }
    };

    let session_cookie = normalize_cookie_input(&cookie_input)?;
    let client = ApiClient::new(base_url.to_string(), Some(session_cookie.clone()))?;
    let me = client.me().context("cookie validation failed")?;
    save_session(&SessionState {
        base_url: client.base_url().to_string(),
        session_cookie,
    })?;
    println!("cookie login complete for {}", me.email);
    Ok(())
}

fn logout() -> Result<()> {
    if let Some(session) = load_session()? {
        let client = ApiClient::from_session(session)?;
        let _ = client.logout();
    }
    clear_session()?;
    println!("logged out");
    Ok(())
}

fn me() -> Result<()> {
    let client = session_client()?;
    let me = client.me()?;
    println!("LOGGED IN AS : {}", me.email);
    println!("BALANCE      : {}", me.balance);
    println!("MINTED       : {}", me.minted);
    println!("SENT         : {}", me.sent);
    println!("RECEIVED     : {}", me.received);
    Ok(())
}

fn mine(args: MineArgs) -> Result<()> {
    let client = session_client()?;
    let running = Arc::new(AtomicBool::new(true));
    let signal = running.clone();
    ctrlc::set_handler(move || {
        signal.store(false, Ordering::SeqCst);
    })
    .context("failed to install ctrl-c handler")?;

    let mut minted = 0u64;
    while running.load(Ordering::SeqCst) {
        let challenge = client.challenge()?;
        let prefix = hex::decode(&challenge.nonce_prefix).context("invalid nonce_prefix from server")?;
        println!(
            "mining challenge {} at {} bits (expires {})",
            challenge.challenge_id, challenge.difficulty_bits, challenge.expires_at
        );

        let started = Instant::now();
        let mut nonce = 0u64;
        let mut last_report = Instant::now();
        let mut last_reported_second = 0u64;

        while running.load(Ordering::SeqCst) {
            if verify_solution(&prefix, nonce, challenge.difficulty_bits) {
                let elapsed = started.elapsed();
                let response = client.mint(&MintRequestBody {
                    challenge_id: challenge.challenge_id.clone(),
                    solution_nonce: nonce.to_string(),
                })?;
                minted += 1;
                let hashes = nonce + 1;
                let rate = hashes as f64 / elapsed.as_secs_f64().max(0.001);
                println!(
                    "minted token {} value={} issued_at={} hashes={} elapsed={:.2}s rate={:.2} H/s",
                    response.token.id,
                    response.token.value,
                    response.token.issued_at,
                    hashes,
                    elapsed.as_secs_f64(),
                    rate
                );
                break;
            }

            nonce = nonce.wrapping_add(1);
            if last_report.elapsed().as_millis() >= 500 {
                let elapsed = started.elapsed();
                let elapsed_secs = elapsed.as_secs();
                if elapsed_secs > last_reported_second && elapsed_secs % 5 == 0 {
                    let elapsed_f = elapsed.as_secs_f64();
                    let rate = nonce as f64 / elapsed_f.max(0.001);
                    println!(
                        "progress elapsed={}s hashes={} rate={:.2} H/s",
                        elapsed_secs, nonce, rate
                    );
                    last_reported_second = elapsed_secs;
                }
                last_report = Instant::now();
            }
        }

        if !running.load(Ordering::SeqCst) {
            println!("mining interrupted");
            break;
        }
        if args.once {
            break;
        }
    }

    if !args.once {
        println!("mined {minted} token(s) in this run");
    }
    Ok(())
}

fn send(args: SendArgs) -> Result<()> {
    let client = session_client()?;
    let response = client.send(&SendRequestBody {
        recipient_email: args.to,
        amount: args.amount,
        idempotency_key: Uuid::new_v4().to_string(),
    })?;
    if !response.ok {
        anyhow::bail!("server reported send failure");
    }
    if response.pending.unwrap_or(false) {
        println!(
            "pending claim: {} RPOW reserved for {} (transfer id {})",
            response.transferred, response.recipient_email, response.transfer_id
        );
    } else {
        println!(
            "sent {} RPOW to {} (transfer id {})",
            response.transferred, response.recipient_email, response.transfer_id
        );
    }
    Ok(())
}

fn activity() -> Result<()> {
    let client = session_client()?;
    let items = client.activity()?;
    if items.is_empty() {
        println!("(no activity yet)");
        return Ok(());
    }
    for item in items {
        println!(
            "{}  {:<8}  {:>4}  {}",
            item.at.replace('T', " ").chars().take(19).collect::<String>(),
            item.r#type.to_uppercase(),
            match item.r#type.as_str() {
                "send" => format!("-{}", item.amount),
                _ => format!("+{}", item.amount),
            },
            item.counterparty_email.unwrap_or_default()
        );
    }
    Ok(())
}

fn ledger(base_url: &str) -> Result<()> {
    let client = ApiClient::new(base_url.to_string(), None)?;
    let ledger = client.ledger()?;
    println!("TOTAL MINTED       : {}", ledger.total_minted);
    println!("TOTAL TRANSFERRED  : {}", ledger.total_transferred);
    println!("CIRCULATING SUPPLY : {}", ledger.circulating_supply);
    println!("CURRENT DIFFICULTY : {} trailing zero bits", ledger.current_difficulty_bits);
    println!("USER COUNT         : {}", ledger.user_count);
    Ok(())
}

fn session_client() -> Result<ApiClient> {
    let session = load_session()?.context("not logged in; run `rpow login --email <email>`")?;
    ApiClient::from_session(session)
}

fn normalize_cookie_input(input: &str) -> Result<String> {
    let trimmed = input.trim();
    let without_prefix = trimmed
        .strip_prefix("Cookie:")
        .or_else(|| trimmed.strip_prefix("cookie:"))
        .or_else(|| trimmed.strip_prefix("Set-Cookie:"))
        .or_else(|| trimmed.strip_prefix("set-cookie:"))
        .unwrap_or(trimmed)
        .trim();

    let extracted = if let Some(idx) = without_prefix.find("rpow_session=") {
        &without_prefix[idx + "rpow_session=".len()..]
    } else {
        without_prefix
    };

    let value = extracted
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .trim_matches('\'');

    let compact: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    let value = compact.trim();
    if value.is_empty() {
        anyhow::bail!("cookie value is empty");
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::normalize_cookie_input;

    #[test]
    fn accepts_raw_cookie_value() {
        assert_eq!(normalize_cookie_input("abc123").unwrap(), "abc123");
    }

    #[test]
    fn accepts_full_cookie_assignment() {
        assert_eq!(
            normalize_cookie_input("rpow_session=abc123; Path=/; HttpOnly").unwrap(),
            "abc123"
        );
    }

    #[test]
    fn accepts_cookie_header_prefix() {
        assert_eq!(
            normalize_cookie_input("Cookie: rpow_session=abc123; theme=dark").unwrap(),
            "abc123"
        );
    }

    #[test]
    fn strips_quotes_and_whitespace() {
        assert_eq!(
            normalize_cookie_input("  \"rpow_session=abc 123 \n\"  ").unwrap(),
            "abc123"
        );
    }
}
