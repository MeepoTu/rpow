use anyhow::{anyhow, bail, Context, Result};
use reqwest::blocking::{Client, Response};
use reqwest::header::{COOKIE, SET_COOKIE};
use reqwest::redirect::Policy;
use url::Url;

use crate::session::SessionState;
use crate::types::{
    ActivityEntry, ApiError, AuthRequestResponse, ChallengeResponse, LedgerResponse, MeResponse,
    MintRequestBody, MintResponse, SendRequestBody, SendResponse,
};

pub struct ApiClient {
    base_url: String,
    client: Client,
    session_cookie: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: String, session_cookie: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .redirect(Policy::none())
            .build()
            .context("failed to build http client")?;
        Ok(Self {
            base_url: normalize_base_url(&base_url)?,
            client,
            session_cookie,
        })
    }

    pub fn from_session(session: SessionState) -> Result<Self> {
        Self::new(session.base_url, Some(session.session_cookie))
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn auth_request(&self, email: &str) -> Result<AuthRequestResponse> {
        self.post_json("/auth/request", &serde_json::json!({ "email": email }))
    }

    pub fn verify_magic_link(&self, link: &str) -> Result<String> {
        let url = parse_magic_link(&self.base_url, link)?;
        let response = self.client.get(url).send().context("failed to verify magic link")?;
        if !response.status().is_redirection() {
            return parse_error_response(response);
        }
        let cookie = extract_session_cookie(&response)
            .context("magic link did not return an rpow_session cookie")?;
        Ok(cookie)
    }

    pub fn logout(&self) -> Result<()> {
        let _ignored: serde_json::Value = self.post_json("/auth/logout", &serde_json::json!({}))?;
        Ok(())
    }

    pub fn me(&self) -> Result<MeResponse> {
        self.get_json_auth("/me")
    }

    pub fn challenge(&self) -> Result<ChallengeResponse> {
        self.post_json_auth("/challenge", &serde_json::json!({}))
    }

    pub fn mint(&self, body: &MintRequestBody) -> Result<MintResponse> {
        self.post_json_auth("/mint", body)
    }

    pub fn send(&self, body: &SendRequestBody) -> Result<SendResponse> {
        self.post_json_auth("/send", body)
    }

    pub fn activity(&self) -> Result<Vec<ActivityEntry>> {
        self.get_json_auth("/activity")
    }

    pub fn ledger(&self) -> Result<LedgerResponse> {
        self.get_json("/ledger")
    }

    fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let response = self
            .client
            .get(self.url(path)?)
            .send()
            .with_context(|| format!("GET {} failed", path))?;
        parse_json_response(response)
    }

    fn get_json_auth<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let response = self
            .client
            .get(self.url(path)?)
            .header(COOKIE, self.cookie_header()?)
            .send()
            .with_context(|| format!("GET {} failed", path))?;
        parse_json_response(response)
    }

    fn post_json<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let response = self
            .client
            .post(self.url(path)?)
            .json(body)
            .send()
            .with_context(|| format!("POST {} failed", path))?;
        parse_json_response(response)
    }

    fn post_json_auth<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let response = self
            .client
            .post(self.url(path)?)
            .header(COOKIE, self.cookie_header()?)
            .json(body)
            .send()
            .with_context(|| format!("POST {} failed", path))?;
        parse_json_response(response)
    }

    fn cookie_header(&self) -> Result<String> {
        let cookie = self
            .session_cookie
            .as_ref()
            .context("not logged in; run `rpow login --email <email>`")?;
        Ok(format!("rpow_session={cookie}"))
    }

    fn url(&self, path: &str) -> Result<Url> {
        Url::parse(&format!("{}{}", self.base_url, path)).context("failed to build request url")
    }
}

fn normalize_base_url(input: &str) -> Result<String> {
    let parsed = Url::parse(input).with_context(|| format!("invalid base url: {input}"))?;
    let mut normalized = parsed.to_string();
    while normalized.ends_with('/') {
        normalized.pop();
    }
    Ok(normalized)
}

fn parse_json_response<T: serde::de::DeserializeOwned>(response: Response) -> Result<T> {
    if response.status().is_success() {
        return response.json().context("failed to decode json response");
    }
    parse_error_response(response)
}

fn parse_error_response<T>(response: Response) -> Result<T> {
    let status = response.status();
    let text = response.text().unwrap_or_default();
    if let Ok(api_error) = serde_json::from_str::<ApiError>(&text) {
        let mut message = format!("{}: {}", api_error.error, api_error.message);
        if let Some(retry_after) = api_error.retry_after {
            message.push_str(&format!(" (retry after {retry_after}s)"));
        }
        bail!(message);
    }
    bail!("http {}: {}", status.as_u16(), text);
}

fn extract_session_cookie(response: &Response) -> Result<String> {
    for header in response.headers().get_all(SET_COOKIE).iter() {
        let value = header.to_str().context("invalid set-cookie header")?;
        if let Some(rest) = value.strip_prefix("rpow_session=") {
            let cookie_value = rest
                .split(';')
                .next()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow!("missing session cookie value"))?;
            return Ok(cookie_value.to_string());
        }
    }
    Err(anyhow!("session cookie not found"))
}

pub fn parse_magic_link(base_url: &str, link: &str) -> Result<Url> {
    let parsed = Url::parse(link).context("magic link is not a valid URL")?;
    let expected = Url::parse(base_url).context("invalid configured base url")?;
    if parsed.scheme() != expected.scheme()
        || parsed.host_str() != expected.host_str()
        || parsed.port_or_known_default() != expected.port_or_known_default()
    {
        bail!("magic link host does not match configured server");
    }
    if parsed.path() != "/auth/verify" {
        bail!("magic link path must be /auth/verify");
    }
    let has_token = parsed.query_pairs().any(|(k, v)| k == "token" && !v.is_empty());
    if !has_token {
        bail!("magic link is missing token parameter");
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_magic_link() {
        let link = "http://localhost:8080/auth/verify?token=abc";
        let parsed = parse_magic_link("http://localhost:8080", link).unwrap();
        assert_eq!(parsed.path(), "/auth/verify");
    }

    #[test]
    fn rejects_wrong_host() {
        let err = parse_magic_link("http://localhost:8080", "http://example.com/auth/verify?token=abc")
            .unwrap_err();
        assert!(err.to_string().contains("host does not match"));
    }
}
