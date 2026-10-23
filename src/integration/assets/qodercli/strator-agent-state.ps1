# installed by strator
# managed by strator; reinstalling or updating the integration overwrites this file.
# add custom hooks beside this file instead of editing it.
# STRATOR_INTEGRATION_ID=qodercli
# STRATOR_INTEGRATION_VERSION=3

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

if ($null -eq $payload -or [string]::IsNullOrWhiteSpace($payload.session_id)) { exit 0 }

$seq = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$strator = if ([string]::IsNullOrWhiteSpace($env:STRATOR_BIN_PATH)) { "strator" } else { $env:STRATOR_BIN_PATH }
try {
    & $strator pane report-agent-session $env:STRATOR_PANE_ID --source strator:qodercli --agent qodercli --agent-session-id $payload.session_id --seq $seq 2>$null | Out-Null
} catch {
}
