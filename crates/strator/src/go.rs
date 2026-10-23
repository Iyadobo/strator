// Protocol snapshot from https://models.dev/api.json, opencode-go, 2026-10-08.
// Unknown models are deliberately not guessed for protocol-specific harnesses.
pub fn supports(harness: &str, model: &str) -> bool {
    if ["opencode", "pi"].contains(&harness) {
        return true;
    }
    let formats: std::collections::BTreeMap<String, String> =
        serde_json::from_str(include_str!("go-models.json")).expect("bundled Go protocol catalog");
    matches!(
        (harness, formats.get(model).map(String::as_str)),
        ("claude", Some("messages")) | ("codex", Some("responses"))
    )
}
