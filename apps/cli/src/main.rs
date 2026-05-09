mod api;
mod pow;
mod session;
mod types;

use std::io::{self, Write};
use std::sync::mpsc;
use std::thread::sleep;
use std::thread;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use uuid::Uuid;

use crate::api::ApiClient;
use crate::pow::verify_solution;
use crate::session::{clear_session, load_session, load_session_from_env, save_session, SessionState};
use crate::types::{MintRequestBody, SendRequestBody};

const DEFAULT_BASE_URL: &str = "http://localhost:8080";
const RETRY_DELAY_SECONDS: u64 = 5;

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
    #[arg(long, default_value_t = 1)]
    cores: usize,
}

#[derive(Debug, Args)]
struct SendArgs {
    #[arg(long)]
    to: String,
    #[arg(long, help = "Amount in RPOW; supports up to 9 decimal places")]
    amount: String,
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
        Commands::Me => me(&cli.base_url),
        Commands::Mine(args) => mine(&cli.base_url, args),
        Commands::Send(args) => send(&cli.base_url, args),
        Commands::Activity => activity(&cli.base_url),
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
    println!("cookie login complete for {}", me.email());
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

fn me(base_url: &str) -> Result<()> {
    let client = session_client(base_url)?;
    let me = client.me()?;
    println!("LOGGED IN AS : {}", me.email());
    println!("BALANCE      : {}", me.balance_display());
    println!("MINTED       : {}", me.minted_display());
    println!("SENT         : {}", me.sent_display());
    println!("RECEIVED     : {}", me.received_display());
    if let Some(wrap_allowed) = me.wrap_allowed() {
        println!("WRAP ALLOWED : {}", if wrap_allowed { "yes" } else { "no" });
    }
    if let Some(wallet) = me.solana_wallet() {
        println!("SOLANA WALLET: {}", wallet);
    }
    if let Some(wrapped) = me.wrapped_supply_display() {
        println!("WRAPPED      : {}", wrapped);
    }
    Ok(())
}

fn mine(base_url: &str, args: MineArgs) -> Result<()> {
    if args.cores == 0 {
        anyhow::bail!("--cores must be at least 1");
    }

    let client = session_client(base_url)?;
    let running = Arc::new(AtomicBool::new(true));
    let signal = running.clone();
    ctrlc::set_handler(move || {
        signal.store(false, Ordering::SeqCst);
    })
    .context("failed to install ctrl-c handler")?;

    let mut minted = 0u64;
    while running.load(Ordering::SeqCst) {
        let challenge = match client.challenge() {
            Ok(challenge) => challenge,
            Err(err) => {
                if args.once {
                    return Err(err);
                }
                println!("challenge error: {err}. retrying in {RETRY_DELAY_SECONDS}s");
                retry_sleep(&running);
                continue;
            }
        };
        let prefix = match hex::decode(&challenge.nonce_prefix)
            .context("invalid nonce_prefix from server")
        {
            Ok(prefix) => prefix,
            Err(err) => {
                if args.once {
                    return Err(err);
                }
                println!("challenge decode error: {err}. retrying in {RETRY_DELAY_SECONDS}s");
                retry_sleep(&running);
                continue;
            }
        };
        println!(
            "mining challenge {} at {} bits (expires {}) with {} core(s)",
            challenge.challenge_id, challenge.difficulty_bits, challenge.expires_at, args.cores
        );

        let solved = solve_challenge(&prefix, challenge.difficulty_bits, args.cores, &running);
        let Some((solution_nonce, hashes, elapsed)) = solved else {
            println!("mining interrupted");
            break;
        };

        let response = match client.mint(&MintRequestBody {
            challenge_id: challenge.challenge_id.clone(),
            solution_nonce: solution_nonce.to_string(),
        }) {
            Ok(response) => response,
            Err(err) => {
                if args.once {
                    return Err(err);
                }
                println!("mint error: {err}. retrying in {RETRY_DELAY_SECONDS}s");
                retry_sleep(&running);
                continue;
            }
        };
        minted += 1;
        let rate_mhs = (hashes as f64 / elapsed.as_secs_f64().max(0.001)) / 1_000_000.0;
        println!(
            "minted token {} value={} issued_at={} hashes={} elapsed={:.2}s rate={:.2} MH/s",
            response.token.id(),
            response.token.value_display(),
            response.token.issued_at(),
            hashes,
            elapsed.as_secs_f64(),
            rate_mhs
        );

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

fn send(base_url: &str, args: SendArgs) -> Result<()> {
    let client = session_client(base_url)?;
    let response = client.send(&SendRequestBody::from_rpow_amount(
        args.to,
        &args.amount,
        Uuid::new_v4().to_string(),
    )?)?;
    if !response.ok() {
        anyhow::bail!("server reported send failure");
    }
    if response.pending() {
        println!(
            "pending claim: {} RPOW reserved for {} (transfer id {})",
            response.transferred_display(),
            response.recipient_email(),
            response.transfer_id()
        );
    } else {
        println!(
            "sent {} RPOW to {} (transfer id {})",
            response.transferred_display(),
            response.recipient_email(),
            response.transfer_id()
        );
    }
    Ok(())
}

fn activity(base_url: &str) -> Result<()> {
    let client = session_client(base_url)?;
    let items = client.activity()?;
    if items.is_empty() {
        println!("(no activity yet)");
        return Ok(());
    }
    for item in items {
        let type_name = item.type_name();
        println!(
            "{}  {:<8}  {:>4}  {}",
            item.at().replace('T', " ").chars().take(19).collect::<String>(),
            type_name.to_uppercase(),
            match type_name {
                "send" => format!("-{}", item.amount_display()),
                _ => format!("+{}", item.amount_display()),
            },
            item.counterparty_email().unwrap_or_default()
        );
    }
    Ok(())
}

fn ledger(base_url: &str) -> Result<()> {
    let client = ApiClient::new(base_url.to_string(), None)?;
    let ledger = client.ledger()?;
    println!("TOTAL MINTED       : {}", ledger.total_minted_display());
    println!("TOTAL TRANSFERRED  : {}", ledger.total_transferred_display());
    println!("CIRCULATING SUPPLY : {}", ledger.circulating_supply_display());
    if let Some(counter) = ledger.minted_counter_display() {
        println!("MINT COUNTER       : {}", counter);
    }
    if let Some(max_supply) = ledger.max_supply_display() {
        println!("MAX SUPPLY         : {}", max_supply);
    }
    println!(
        "CURRENT DIFFICULTY : {} trailing zero bits",
        ledger.current_difficulty_bits()
    );
    if let Some(reward) = ledger.current_reward_display() {
        println!("CURRENT REWARD     : {}", reward);
    }
    if let Some(reward) = ledger.next_reward_display() {
        println!("NEXT REWARD        : {}", reward);
    }
    if let Some(next_halving) = ledger.next_halving_at_display() {
        println!("NEXT HALVING AT    : {}", next_halving);
    }
    if let Some(remaining) = ledger.units_to_next_halving_display() {
        println!("TO NEXT HALVING    : {}", remaining);
    }
    if let Some(halving_index) = ledger.halving_index() {
        println!("HALVING INDEX      : {}", halving_index);
    }
    if let Some(is_capped) = ledger.is_capped() {
        println!("SUPPLY CAPPED      : {}", if is_capped { "yes" } else { "no" });
    }
    println!("USER COUNT         : {}", ledger.user_count());
    Ok(())
}

fn session_client(base_url: &str) -> Result<ApiClient> {
    let session = load_session_from_env(Some(base_url))
        .or(load_session()?)
        .context(
            "not logged in; run `rpow login --email <email>`, `rpow cookie-login`, or set RPOW_SESSION_COOKIE",
        )?;
    ApiClient::from_session(session)
}

fn retry_sleep(running: &AtomicBool) {
    for _ in 0..RETRY_DELAY_SECONDS {
        if !running.load(Ordering::SeqCst) {
            return;
        }
        sleep(Duration::from_secs(1));
    }
}

fn solve_challenge(
    prefix: &[u8],
    difficulty_bits: u32,
    cores: usize,
    running: &Arc<AtomicBool>,
) -> Option<(u64, u64, Duration)> {
    let started = Instant::now();
    let total_hashes = Arc::new(AtomicU64::new(0));
    let solved = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();

    thread::scope(|scope| {
        for worker_id in 0..cores {
            let tx = tx.clone();
            let prefix = prefix.to_vec();
            let running = running.clone();
            let solved = solved.clone();
            let total_hashes = total_hashes.clone();
            scope.spawn(move || {
                let mut nonce = worker_id as u64;
                let stride = cores as u64;
                let mut local_hashes = 0u64;

                while running.load(Ordering::Relaxed) && !solved.load(Ordering::Relaxed) {
                    if verify_solution(&prefix, nonce, difficulty_bits) {
                        total_hashes.fetch_add(local_hashes + 1, Ordering::Relaxed);
                        solved.store(true, Ordering::Relaxed);
                        let _ = tx.send(nonce);
                        return;
                    }

                    nonce = nonce.wrapping_add(stride);
                    local_hashes += 1;
                    if local_hashes >= 4096 {
                        total_hashes.fetch_add(local_hashes, Ordering::Relaxed);
                        local_hashes = 0;
                    }
                }

                if local_hashes > 0 {
                    total_hashes.fetch_add(local_hashes, Ordering::Relaxed);
                }
            });
        }

        drop(tx);

        let mut last_reported_second = 0u64;
        loop {
            if !running.load(Ordering::SeqCst) {
                solved.store(true, Ordering::SeqCst);
                return None;
            }

            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(solution_nonce) => {
                    let elapsed = started.elapsed();
                    let hashes = total_hashes.load(Ordering::Relaxed);
                    return Some((solution_nonce, hashes.max(1), elapsed));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let elapsed = started.elapsed();
                    let elapsed_secs = elapsed.as_secs();
                    if elapsed_secs > last_reported_second && elapsed_secs % 5 == 0 {
                        let hashes = total_hashes.load(Ordering::Relaxed);
                        let rate_mhs =
                            (hashes as f64 / elapsed.as_secs_f64().max(0.001)) / 1_000_000.0;
                        println!(
                            "progress elapsed={}s hashes={} rate={:.2} MH/s",
                            elapsed_secs, hashes, rate_mhs
                        );
                        last_reported_second = elapsed_secs;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if !running.load(Ordering::SeqCst) {
                        return None;
                    }
                    return None;
                }
            }
        }
    })
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
