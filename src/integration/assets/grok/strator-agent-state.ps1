# installed by strator
# managed by strator; reinstalling or updating the integration overwrites this file.
# add custom hooks beside this file instead of editing it.
# STRATOR_INTEGRATION_ID=grok
# STRATOR_INTEGRATION_VERSION=2

param([string]$Action = "")

if ($Action -ne "session") { exit 0 }
if ($env:STRATOR_ENV -ne "1") { exit 0 }
if ([string]::IsNullOrWhiteSpace($env:STRATOR_PANE_ID)) { exit 0 }

$inputText = [Console]::In.ReadToEnd()
try {
    $payload = if ([string]::IsNullOrWhiteSpace($inputText)) { $null } else { $inputText | ConvertFrom-Json }
} catch {
    $payload = $null
}

$event = if ($null -ne $payload -and $payload.hook_event_name -is [string]) {
    $payload.hook_event_name
} elseif ($null -ne $payload -and $payload.hookEventName -is [string]) {
    $payload.hookEventName
} else {
    $null
}
if ($null -ne $event -and $event -notin @("session_start", "SessionStart", "sessionStart")) { exit 0 }

$sessionStartSource = if ($null -ne $payload -and $payload.source -is [string]) {
    $payload.source
} else {
    $null
}

$sessionId = $env:GROK_SESSION_ID
if ([string]::IsNullOrWhiteSpace($sessionId) -and $null -ne $payload) {
    if ($payload.session_id -is [string]) { $sessionId = $payload.session_id }
    elseif ($payload.sessionId -is [string]) { $sessionId = $payload.sessionId }
}
if ([string]::IsNullOrWhiteSpace($sessionId)) { exit 0 }

$seq = [DateTime]::UtcNow.Ticks
$strator = if ([string]::IsNullOrWhiteSpace($env:STRATOR_BIN_PATH)) { "strator" } else { $env:STRATOR_BIN_PATH }
$stratorArgs = @(
    "pane", "report-agent-session", $env:STRATOR_PANE_ID,
    "--source", "strator:grok",
    "--agent", "grok",
    "--seq", "$seq",
    "--agent-session-id", "$sessionId"
)
if (-not [string]::IsNullOrWhiteSpace($sessionStartSource)) {
    $stratorArgs += @("--session-start-source", "$sessionStartSource")
}
try {
    & $strator @stratorArgs 2>$null | Out-Null
} catch {
}
