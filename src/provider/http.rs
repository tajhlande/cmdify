use reqwest::RequestBuilder;

use crate::debug;
use crate::debug_json;
use crate::error::{Error, Result};

pub(crate) async fn send_and_parse<F>(
    request: RequestBuilder,
    url: &str,
    parse: F,
) -> Result<crate::provider::ProviderResponse>
where
    F: FnOnce(&serde_json::Value) -> Result<crate::provider::ProviderResponse>,
{
    debug!("Sending request to {}", url);

    let start = std::time::Instant::now();
    let response = request.send().await?;
    let elapsed = start.elapsed().as_millis();

    let status = response.status();
    let raw_body = response.text().await?;

    debug!(
        "Received response (status {} in {}ms)",
        status.as_u16(),
        elapsed
    );

    let body: serde_json::Value = match serde_json::from_str(&raw_body) {
        Ok(v) => v,
        Err(e) => {
            debug!("Failed to decode response body as JSON: {}", e);
            debug!("Raw response body:\n{}", raw_body);
            return Err(Error::ProviderError(format!(
                "error decoding response body: {}",
                e
            )));
        }
    };

    if !status.is_success() {
        debug_json!("Error response", &body);
        let error_msg = body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Err(Error::ProviderError(format!(
            "API returned {} {}: {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown"),
            error_msg,
        )));
    }

    debug_json!("Response body", &body);

    parse(&body)
}

pub(crate) fn user_agent() -> String {
    format!("cmdify/{}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_contains_cmdify() {
        let ua = user_agent();
        assert!(ua.starts_with("cmdify/"));
    }
}
