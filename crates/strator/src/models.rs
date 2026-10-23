use crate::catalog;
use reqwest::blocking::Client;
use serde_json::Value;
use std::io::{self, Read};
use std::time::Duration;

fn parse(provider: &str, value: &Value) -> Vec<String> {
    let field = if provider == "google" {
        "models"
    } else {
        "data"
    };
    let mut models: Vec<_> = value[field]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            if provider == "google" {
                if !m["supportedGenerationMethods"]
                    .as_array()?
                    .iter()
                    .any(|v| v == "generateContent")
                {
                    return None;
                }
                Some(m["name"].as_str()?.trim_start_matches("models/").to_owned())
            } else {
                let id = m["id"].as_str()?;
                if m["lifecycle"] == "retired" {
                    return None;
                }
                // OpenAI also returns image, speech and embedding models; these are not coding models.
                if provider == "openai"
                    && !id.starts_with("gpt-")
                    && !id.starts_with("chatgpt-")
                    && !id.starts_with("o1")
                    && !id.starts_with("o3")
                    && !id.starts_with("o4")
                {
                    return None;
                }
                Some(id.to_owned())
            }
        })
        .filter(|id| !id.is_empty() && !id.chars().any(char::is_control))
        .collect();
    models.sort();
    models.dedup();
    models
}
pub fn fetch(provider: &str, endpoint: &str, key: &str) -> io::Result<Vec<String>> {
    if provider == "opencode-go" {
        let spec = catalog::LaunchSpec {
            command: "opencode".into(),
            args: vec!["models".into(), "opencode-go".into()],
            env: Default::default(),
            cwd: std::env::current_dir()?.to_string_lossy().into_owned(),
        };
        let result = crate::platform::output_spec(&spec)?;
        if !result.status.success() {
            return Err(io::Error::other(
                "OpenCode could not list Go models. Enter a model ID manually.",
            ));
        }
        let mut ids: Vec<_> = String::from_utf8_lossy(&result.stdout)
            .lines()
            .filter_map(|line| line.trim().strip_prefix("opencode-go/"))
            .filter(|id| !id.is_empty() && !id.chars().any(char::is_control))
            .map(str::to_owned)
            .collect();
        ids.sort();
        ids.dedup();
        return Ok(ids);
    }
    let base = match provider {
        "anthropic" if endpoint.is_empty() => "https://api.anthropic.com",
        "google" if endpoint.is_empty() => "https://generativelanguage.googleapis.com/v1beta",
        _ => endpoint,
    };
    catalog::validate_endpoint(base).map_err(io::Error::other)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| io::Error::other("Could not start model lookup."))?;
    let base = base.trim_end_matches('/');
    let url = if ["anthropic", "gateway"].contains(&provider) && !base.ends_with("/v1") {
        format!("{base}/v1/models")
    } else {
        format!("{base}/models")
    };
    let mut models = Vec::new();
    let mut cursor = None;
    for _ in 0..20 {
        let mut request = client.get(&url);
        if ["anthropic", "gateway"].contains(&provider) {
            request = request
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .query(&[("limit", "1000")]);
            if let Some(ref c) = cursor {
                request = request.query(&[("after_id", c)]);
            }
        } else if provider == "google" {
            request = request
                .header("x-goog-api-key", key)
                .query(&[("pageSize", "1000")]);
            if let Some(ref c) = cursor {
                request = request.query(&[("pageToken", c)]);
            }
        } else {
            request = request.bearer_auth(key);
        }
        let response = request
            .send()
            .map_err(|_| io::Error::other("Model lookup failed (network or timeout)."))?;
        if !response.status().is_success() {
            return Err(io::Error::other(format!(
                "Model lookup returned HTTP {}.",
                response.status().as_u16()
            )));
        }
        // Never print provider responses: error bodies can contain credentials.
        let mut bytes = Vec::new();
        response.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(io::Error::other("Model list is too large."));
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| io::Error::other("Provider returned an invalid model list."))?;
        models.extend(parse(provider, &value));
        let next = if provider == "google" {
            value["nextPageToken"].as_str()
        } else if value["has_more"] == true {
            value["last_id"].as_str()
        } else {
            None
        };
        match next {
            Some(c) if cursor.as_deref() != Some(c) => cursor = Some(c.to_owned()),
            _ => break,
        }
    }
    models.sort();
    models.dedup();
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn filters_non_chat_google_models_and_normalizes_ids() {
        assert_eq!(
            parse(
                "google",
                &json!({"models":[{"name":"models/gemini-test","supportedGenerationMethods":["generateContent"]},{"name":"models/embed","supportedGenerationMethods":["embedContent"]}]})
            ),
            vec!["gemini-test"]
        );
    }
    #[test]
    fn excludes_retired_models_and_deduplicates() {
        assert_eq!(
            parse(
                "anthropic",
                &json!({"data":[{"id":"claude-test"},{"id":"claude-test"},{"id":"old","lifecycle":"retired"}]})
            ),
            vec!["claude-test"]
        );
    }
    #[test]
    fn local_provider_receives_auth_and_returns_models() {
        use std::io::Write;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut bytes = [0; 4096];
            let n = socket.read(&mut bytes).unwrap();
            let request = String::from_utf8_lossy(&bytes[..n]).to_lowercase();
            assert!(request.starts_with("get /v1/models "));
            assert!(request.contains("authorization: bearer fake-key"));
            let body = r#"{"data":[{"id":"test-model"}]}"#;
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        assert_eq!(
            fetch("compatible", &endpoint, "fake-key").unwrap(),
            vec!["test-model"]
        );
        server.join().unwrap();
    }
}
