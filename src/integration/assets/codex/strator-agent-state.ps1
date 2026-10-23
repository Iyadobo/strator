# installed by strator
# managed by strator; reinstalling or updating the integration overwrites this file.
# add custom hooks beside this file instead of editing it.
# STRATOR_INTEGRATION_ID=codex
# STRATOR_INTEGRATION_VERSION=9

param([string]$Action = "")

if ($Action -notin @("session", "working", "idle")) { exit 0 }
if ($env:STRATOR_ENV -ne "1") { exit 0 }
if ([string]::IsNullOrWhiteSpace($env:STRATOR_PANE_ID)) { exit 0 }

$inputText = [Console]::In.ReadToEnd()
try {
    $payload = if ([string]::IsNullOrWhiteSpace($inputText)) { $null } else { $inputText | ConvertFrom-Json }
} catch {
    exit 0
}

$expectedEvents = @{ session = @("SessionStart"); working = @("UserPromptSubmit"); idle = @("Stop", "Interrupt") }[$Action]
if ($payload.hook_event_name -and $payload.hook_event_name -notin $expectedEvents) { exit 0 }

$sessionId = $payload.session_id
if ([string]::IsNullOrWhiteSpace($sessionId)) { exit 0 }
if ($Action -eq "session" -and [string]::IsNullOrWhiteSpace($payload.transcript_path)) { exit 0 }
if (-not [string]::IsNullOrWhiteSpace($env:CODEX_THREAD_ID) -and $env:CODEX_THREAD_ID -ne $sessionId) { exit 0 }

$seq = [DateTimeOffset]::UtcNow.Ticks
$strator = if ([string]::IsNullOrWhiteSpace($env:STRATOR_BIN_PATH)) { "strator" } else { $env:STRATOR_BIN_PATH }
try {
    $args = @(
        "pane",
        $(if ($Action -eq "session") { "report-agent-session" } else { "report-agent" }),
        $env:STRATOR_PANE_ID,
        "--source",
        "strator:codex",
        "--agent",
        "codex",
        "--seq",
        "$seq",
        "--agent-session-id",
        "$sessionId"
    )
    if ($Action -ne "session") { $args += @("--state", $Action) }
    if ($payload.hook_event_name -eq "SessionStart" -and $payload.source -is [string] -and -not [string]::IsNullOrWhiteSpace($payload.source)) {
        $args += @("--session-start-source", "$($payload.source)")
    }
    & $strator @args 2>$null | Out-Null
} catch {
}
