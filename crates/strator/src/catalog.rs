use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub key_env: &'static str,
    pub endpoint: &'static str,
}
pub const PROVIDERS: &[Provider] = &[
    Provider {
        id: "opencode-go",
        name: "OpenCode Go",
        key_env: "OPENCODE_API_KEY",
        endpoint: "https://opencode.ai/zen/go/v1",
    },
    Provider {
        id: "native",
        name: "Harness login / existing setup",
        key_env: "",
        endpoint: "",
    },
    Provider {
        id: "anthropic",
        name: "Anthropic · Messages API",
        key_env: "ANTHROPIC_API_KEY",
        endpoint: "",
    },
    Provider {
        id: "openai",
        name: "OpenAI",
        key_env: "OPENAI_API_KEY",
        endpoint: "https://api.openai.com/v1",
    },
    Provider {
        id: "google",
        name: "Google AI Studio · Gemini API",
        key_env: "GEMINI_API_KEY",
        endpoint: "",
    },
    Provider {
        id: "openrouter",
        name: "OpenRouter · Chat Completions API",
        key_env: "OPENROUTER_API_KEY",
        endpoint: "https://openrouter.ai/api/v1",
    },
    Provider {
        id: "compatible",
        name: "Custom · Chat Completions API",
        key_env: "STRATOR_API_KEY",
        endpoint: "",
    },
    Provider {
        id: "responses",
        name: "Custom · Responses API",
        key_env: "STRATOR_API_KEY",
        endpoint: "",
    },
    Provider {
        id: "gateway",
        name: "Custom · Anthropic Messages gateway",
        key_env: "STRATOR_API_KEY",
        endpoint: "",
    },
];
pub struct Harness {
    pub id: &'static str,
    pub name: &'static str,
    pub command: &'static str,
    pub providers: &'static [&'static str],
    pub install: &'static str,
    pub model_flag: bool,
}
pub const HARNESSES: &[Harness] = &[
    Harness {
        id: "claude",
        name: "Claude Code",
        command: "claude",
        providers: &["native", "anthropic", "gateway", "opencode-go"],
        install: "npm install -g @anthropic-ai/claude-code",
        model_flag: true,
    },
    Harness {
        id: "codex",
        name: "Codex",
        command: "codex",
        providers: &["native", "openai", "responses", "opencode-go"],
        install: "npm install -g @openai/codex",
        model_flag: true,
    },
    Harness {
        id: "opencode",
        name: "OpenCode",
        command: "opencode",
        providers: &[
            "native",
            "opencode-go",
            "anthropic",
            "openai",
            "openrouter",
            "compatible",
            "responses",
        ],
        install: "npm install -g opencode-ai",
        model_flag: true,
    },
    Harness {
        id: "gemini",
        name: "Gemini CLI",
        command: "gemini",
        providers: &["native", "google"],
        install: "npm install -g @google/gemini-cli",
        model_flag: true,
    },
    Harness {
        id: "aider",
        name: "Aider",
        command: "aider",
        providers: &["native", "anthropic", "openai", "openrouter", "compatible"],
        install: "uv tool install --python python3.12 --with pip aider-chat@latest",
        model_flag: true,
    },
    Harness {
        id: "pi",
        name: "Pi",
        command: "pi",
        providers: &[
            "native",
            "anthropic",
            "openai",
            "google",
            "openrouter",
            "opencode-go",
        ],
        install: "npm install -g --ignore-scripts @earendil-works/pi-coding-agent",
        model_flag: true,
    },
    Harness {
        id: "goose",
        name: "Goose",
        command: "goose",
        providers: &["native"],
        install: "https://block.github.io/goose/docs/getting-started/installation/",
        model_flag: false,
    },
    Harness {
        id: "amp",
        name: "Amp",
        command: "amp",
        providers: &["native"],
        install: "npm install -g @sourcegraph/amp",
        model_flag: false,
    },
    Harness {
        id: "copilot",
        name: "GitHub Copilot CLI",
        command: "copilot",
        providers: &["native"],
        install: "npm install -g @github/copilot",
        model_flag: true,
    },
    Harness {
        id: "cursor",
        name: "Cursor Agent",
        command: "agent",
        providers: &["native"],
        install: "https://cursor.com/docs/cli/installation",
        model_flag: true,
    },
    Harness {
        id: "terminal",
        name: "Terminal",
        command: "",
        providers: &["native"],
        install: "Included with your operating system",
        model_flag: false,
    },
];

#[derive(Serialize, Deserialize)]
pub struct LaunchSpec {
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: String,
}

pub fn build(
    harness: &Harness,
    provider: &Provider,
    model: &str,
    endpoint: &str,
    key_env: &str,
    key: Option<String>,
    cwd: String,
) -> Result<LaunchSpec, String> {
    if !harness.providers.contains(&provider.id) {
        return Err(format!(
            "{} does not support {}. Choose a matching provider.",
            harness.name, provider.name
        ));
    }
    if provider.id == "opencode-go" && ["claude", "codex"].contains(&harness.id) {
        if !crate::go::supports(harness.id, model) {
            return Err(format!("{} cannot use this Go model's API format. Choose a compatible model, OpenCode, or Pi.", harness.name));
        }
        let adapter_id = if harness.id == "claude" {
            "gateway"
        } else {
            "responses"
        };
        let adapter = PROVIDERS.iter().find(|p| p.id == adapter_id).unwrap();
        let base = if endpoint.is_empty() {
            provider.endpoint
        } else {
            endpoint
        };
        let base = if harness.id == "claude" {
            base.trim_end_matches('/').trim_end_matches("/v1")
        } else {
            base
        };
        return build(harness, adapter, model, base, key_env, key, cwd);
    }
    if model.chars().any(char::is_control) || model.len() > 200 {
        return Err("Invalid model ID.".into());
    }
    let mut spec = LaunchSpec {
        command: harness.command.into(),
        args: vec![],
        env: BTreeMap::new(),
        cwd,
    };
    if provider.id == "native" {
        if !model.is_empty() {
            if !harness.model_flag {
                return Err("This harness chooses its model through its own configuration.".into());
            }
            spec.args.extend(["--model".into(), model.into()]);
        }
        return Ok(spec);
    }
    if key_env.is_empty()
        || !key_env
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        || !key_env.as_bytes()[0].is_ascii_uppercase()
    {
        return Err("Key variable must be an uppercase environment variable name.".into());
    }
    let key = key.filter(|k| !k.is_empty()).ok_or_else(|| {
        format!("Set {key_env} or enter a provider key in the interactive picker.")
    })?;
    if key.chars().any(char::is_control) {
        return Err("Key contains control characters.".into());
    }
    let endpoint = if endpoint.is_empty() {
        provider.endpoint
    } else {
        endpoint
    };
    if ["compatible", "responses", "gateway"].contains(&provider.id) && endpoint.is_empty() {
        return Err("This provider needs --base-url.".into());
    }
    if !endpoint.is_empty() {
        validate_endpoint(endpoint)?;
    }
    spec.env.insert(key_env.into(), key.clone());
    match harness.id {
        "claude" => {
            spec.env.insert("ANTHROPIC_API_KEY".into(), key);
            spec.env
                .insert("ANTHROPIC_AUTH_TOKEN".into(), String::new());
            if !endpoint.is_empty() {
                spec.env
                    .insert("ANTHROPIC_BASE_URL".into(), endpoint.into());
            }
        }
        "codex" => {
            let entries = [
                "model_provider=\"strator\"".to_string(),
                "model_providers.strator.name=\"Strator\"".into(),
                format!("model_providers.strator.base_url={}", json!(endpoint)),
                format!("model_providers.strator.env_key={}", json!(key_env)),
                "model_providers.strator.wire_api=\"responses\"".into(),
                "model_providers.strator.requires_openai_auth=false".into(),
            ];
            for entry in entries {
                spec.args.extend(["-c".into(), entry]);
            }
        }
        "gemini" => {
            spec.env.insert("GEMINI_API_KEY".into(), key);
        }
        "opencode" => {
            if provider.id == "opencode-go" && model.is_empty() {
                return Err("OpenCode Go requires a model selection.".into());
            }
            let config = if ["compatible", "responses"].contains(&provider.id) {
                if model.is_empty() {
                    return Err("Custom OpenCode providers require --model.".into());
                }
                json!({"provider":{"strator":{"npm":if provider.id=="responses"{"@ai-sdk/openai"}else{"@ai-sdk/openai-compatible"},"name":"Strator","options":{"baseURL":endpoint,"apiKey":format!("{{env:{key_env}}}")},"models":{model:{"name":model}}}},"model":format!("strator/{model}")})
            } else {
                let mut options = json!({"apiKey":format!("{{env:{key_env}}}")});
                if !endpoint.is_empty() && provider.id != "opencode-go" {
                    options["baseURL"] = json!(endpoint);
                }
                if !model.is_empty() {
                    spec.args
                        .extend(["--model".into(), format!("{}/{model}", provider.id)]);
                }
                json!({"provider":{provider.id:{"options":options}}})
            };
            spec.env
                .insert("OPENCODE_CONFIG_CONTENT".into(), config.to_string());
            return Ok(spec);
        }
        "aider" => {
            if model.is_empty() {
                return Err("Aider BYOK requires --model.".into());
            }
            if ["compatible", "openai"].contains(&provider.id) {
                spec.env.insert("OPENAI_API_KEY".into(), key);
                if !endpoint.is_empty() {
                    spec.env.insert("OPENAI_API_BASE".into(), endpoint.into());
                }
            } else {
                spec.env.insert(provider.key_env.into(), key);
            }
            spec.args.extend([
                "--model".into(),
                format!(
                    "{}/{model}",
                    if provider.id == "compatible" {
                        "openai"
                    } else {
                        provider.id
                    }
                ),
            ]);
            return Ok(spec);
        }
        "pi" => {
            if model.is_empty() {
                return Err("Pi BYOK requires --model.".into());
            }
            spec.env.insert(provider.key_env.into(), key);
            spec.args.extend(["--provider".into(), provider.id.into()]);
        }
        _ => return Err("No BYOK adapter exists for this harness.".into()),
    }
    if !model.is_empty() {
        spec.args.extend(["--model".into(), model.into()]);
    }
    Ok(spec)
}

pub fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    // Keep credentials out of URLs and permit cleartext only on loopback.
    let (scheme, rest) = endpoint
        .split_once("://")
        .ok_or("Endpoint must be an HTTPS URL.")?;
    let authority = rest.split('/').next().unwrap_or("");
    let local = authority == "localhost"
        || authority == "127.0.0.1"
        || authority == "[::1]"
        || authority.starts_with("localhost:")
        || authority.starts_with("127.0.0.1:")
        || authority.starts_with("[::1]:");
    if authority.is_empty()
        || endpoint.contains(['@', '?', '#'])
        || endpoint
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
        || (scheme != "https" && !(scheme == "http" && local))
    {
        return Err(
            "Use HTTPS, or HTTP on localhost. No credentials or query strings in the URL.".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec(h: &str, p: &str, model: &str, url: &str) -> Result<LaunchSpec, String> {
        build(
            HARNESSES.iter().find(|v| v.id == h).unwrap(),
            PROVIDERS.iter().find(|v| v.id == p).unwrap(),
            model,
            url,
            "TEST_API_KEY",
            Some("fake-test-key".into()),
            ".".into(),
        )
    }
    #[test]
    fn protocols_are_not_interchangeable() {
        assert!(spec("claude", "openai", "", "").is_err());
        assert!(spec("codex", "compatible", "m", "https://example.com/v1").is_err());
    }
    #[test]
    fn claude_key_only_in_environment() {
        let s = spec("claude", "gateway", "m", "https://example.com/anthropic").unwrap();
        assert_eq!(s.env["ANTHROPIC_API_KEY"], "fake-test-key");
        assert!(!s.args.join(" ").contains("fake-test-key"));
    }
    #[test]
    fn codex_responses_adapter() {
        let s = spec("codex", "openai", "m", "").unwrap();
        assert!(s
            .args
            .contains(&"model_providers.strator.wire_api=\"responses\"".into()));
        assert!(s
            .args
            .contains(&"model_providers.strator.env_key=\"TEST_API_KEY\"".into()));
    }
    #[test]
    fn opencode_protocol_and_env_reference() {
        for (p, npm) in [
            ("compatible", "@ai-sdk/openai-compatible"),
            ("responses", "@ai-sdk/openai"),
        ] {
            let s = spec("opencode", p, "model-x", "https://example.com/v1").unwrap();
            let v: serde_json::Value =
                serde_json::from_str(&s.env["OPENCODE_CONFIG_CONTENT"]).unwrap();
            assert_eq!(v["provider"]["strator"]["npm"], npm);
            assert_eq!(
                v["provider"]["strator"]["options"]["apiKey"],
                "{env:TEST_API_KEY}"
            );
            assert!(v["provider"]["strator"]["models"]["model-x"].is_object());
            assert!(!s.env["OPENCODE_CONFIG_CONTENT"].contains("fake-test-key"));
        }
    }
    #[test]
    fn provider_specific_environment() {
        assert_eq!(
            spec("gemini", "google", "", "").unwrap().env["GEMINI_API_KEY"],
            "fake-test-key"
        );
        assert_eq!(
            spec("aider", "openrouter", "vendor/model", "").unwrap().env["OPENROUTER_API_KEY"],
            "fake-test-key"
        );
        assert!(spec("pi", "anthropic", "m", "")
            .unwrap()
            .args
            .contains(&"--provider".into()));
    }
    #[test]
    fn missing_model_and_endpoint_rejected() {
        assert!(spec("opencode", "opencode-go", "", "").is_err());
        assert!(spec("aider", "openai", "", "").is_err());
        assert!(spec("opencode", "compatible", "", "https://example.com/v1").is_err());
        assert!(spec("claude", "gateway", "m", "").is_err());
    }
    #[test]
    fn go_uses_native_provider_and_keeps_model_protocol_metadata() {
        let s = spec(
            "opencode",
            "opencode-go",
            "kimi-k3",
            "https://opencode.ai/zen/go/v1",
        )
        .unwrap();
        assert_eq!(s.args, vec!["--model", "opencode-go/kimi-k3"]);
        let config: serde_json::Value =
            serde_json::from_str(&s.env["OPENCODE_CONFIG_CONTENT"]).unwrap();
        assert_eq!(
            config["provider"]["opencode-go"]["options"]["apiKey"],
            "{env:TEST_API_KEY}"
        );
        assert!(config["provider"]["opencode-go"]["options"]["baseURL"].is_null());
        assert!(config["provider"]["opencode-go"]["npm"].is_null());
        assert!(!s.env["OPENCODE_CONFIG_CONTENT"].contains("fake-test-key"));
    }
    #[test]
    fn go_routes_harnesses_by_model_protocol() {
        let claude = spec("claude", "opencode-go", "claude-haiku-5-5", "").unwrap();
        assert_eq!(
            claude.env["ANTHROPIC_BASE_URL"],
            "https://opencode.ai/zen/go"
        );
        assert_eq!(claude.env["ANTHROPIC_API_KEY"], "fake-test-key");
        let codex = spec("codex", "opencode-go", "gpt-6-luna", "").unwrap();
        assert!(codex
            .args
            .iter()
            .any(|s| s.contains("https://opencode.ai/zen/go/v1")));
        assert!(codex
            .args
            .iter()
            .any(|s| s.contains("wire_api=\"responses\"")));
        let pi = spec("pi", "opencode-go", "kimi-k3", "").unwrap();
        assert_eq!(pi.env["OPENCODE_API_KEY"], "fake-test-key");
        assert_eq!(
            pi.args,
            vec!["--provider", "opencode-go", "--model", "kimi-k3"]
        );
        assert!(spec("claude", "opencode-go", "kimi-k3", "").is_err());
        assert!(spec("codex", "opencode-go", "claude-haiku-5-5", "").is_err());
        assert!(spec("claude", "opencode-go", "unknown-model", "").is_err());
    }
    #[test]
    fn endpoint_rules() {
        for url in [
            "http://remote.example/v1",
            "https://user:pass@example.com",
            "https://example.com?key=secret",
            "http://127.0.0.1.evil.example/v1",
        ] {
            assert!(validate_endpoint(url).is_err());
        }
        assert!(validate_endpoint("http://127.0.0.1:1234/v1").is_ok());
    }
    #[test]
    fn native_does_not_inject_credentials() {
        let s = spec("claude", "native", "", "").unwrap();
        assert!(s.env.is_empty());
        assert!(s.args.is_empty());
    }
}
