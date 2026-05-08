use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub base_url: String,
    pub session_cookie: String,
}

fn config_dir() -> Result<PathBuf> {
    let base = dirs::config_dir().context("could not resolve config directory")?;
    Ok(base.join("rpow"))
}

fn session_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("session.json"))
}

pub fn load_session() -> Result<Option<SessionState>> {
    let path = session_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let session = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(session))
}

pub fn save_session(session: &SessionState) -> Result<()> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create {}", dir.display()))?;
    let path = session_path()?;
    let raw = serde_json::to_string_pretty(session)?;
    fs::write(&path, raw)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn clear_session() -> Result<()> {
    let path = session_path()?;
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_serializes() {
        let session = SessionState {
            base_url: "http://localhost:8080".to_string(),
            session_cookie: "cookie-value".to_string(),
        };
        let raw = serde_json::to_string(&session).unwrap();
        let round_trip: SessionState = serde_json::from_str(&raw).unwrap();
        assert_eq!(round_trip.base_url, session.base_url);
        assert_eq!(round_trip.session_cookie, session.session_cookie);
    }
}
