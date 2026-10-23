use std::io;

// BYOK = model provider key. Entries are scoped to provider + endpoint
// + key variable only, never to a harness: one saved OpenAI key serves
// Codex, OpenCode, Pi, and every other harness launched on OpenAI.
fn entry(provider: &str, endpoint: &str, variable: &str) -> io::Result<keyring::Entry> {
    // Scope gateway keys to the exact endpoint, not just the provider name.
    let account = serde_json::to_string(&(provider, endpoint.trim_end_matches('/'), variable))?;
    keyring::Entry::new("Strator", &account)
        .map_err(|_| io::Error::other("OS credential store is unavailable."))
}
pub fn load(provider: &str, endpoint: &str, variable: &str) -> io::Result<Option<String>> {
    match entry(provider, endpoint, variable)?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(io::Error::other("Could not read the OS credential store.")),
    }
}
pub fn save(provider: &str, endpoint: &str, variable: &str, key: &str) -> io::Result<()> {
    if key.trim().is_empty() {
        return Err(io::Error::other("API key cannot be empty."));
    }
    entry(provider, endpoint, variable)?
        .set_password(key)
        .map_err(|_| io::Error::other("Could not save key in the OS credential store."))
}
pub fn forget(provider: &str, endpoint: &str, variable: &str) -> io::Result<()> {
    match entry(provider, endpoint, variable)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(io::Error::other("Could not delete the saved key.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Writes an isolated test credential into the real OS store"]
    fn os_store_round_trip_and_endpoint_isolation() {
        let provider = format!("test-{}", std::process::id());
        save(&provider, "https://one.invalid/v1", "TEST", "fake-test-key").unwrap();
        assert_eq!(
            load(&provider, "https://one.invalid/v1/", "TEST")
                .unwrap()
                .as_deref(),
            Some("fake-test-key")
        );
        assert!(load(&provider, "https://two.invalid/v1", "TEST")
            .unwrap()
            .is_none());
        forget(&provider, "https://one.invalid/v1", "TEST").unwrap();
        assert!(load(&provider, "https://one.invalid/v1", "TEST")
            .unwrap()
            .is_none());
    }
}
