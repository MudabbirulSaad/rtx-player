param(
    [Parameter(Mandatory)][string]$Player,
    [Parameter(Mandatory)][string]$Media,
    [Parameter(Mandatory)][string]$ExpectedWarning
)
# Use a separately staged application with a deliberately incomplete optional provider.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
$exe = (Resolve-Path -LiteralPath $Player).Path
$clip = (Resolve-Path -LiteralPath $Media).Path
$process = Start-Process -FilePath $exe -ArgumentList ('"' + $clip + '"') -NoNewWindow -PassThru
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $process.Refresh()
        if ($process.MainWindowTitle -eq 'RTX Player') { break }
        Start-Sleep -Milliseconds 100
    } while (!$process.HasExited -and [DateTime]::UtcNow -lt $deadline)
    $root = [Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
    function Find-ByName([string]$name) {
        $condition = [Windows.Automation.PropertyCondition]::new(
            [Windows.Automation.AutomationElement]::NameProperty, $name)
        $timeout = [DateTime]::UtcNow.AddSeconds(6)
        do {
            $node = $root.FindFirst([Windows.Automation.TreeScope]::Descendants, $condition)
            if ($node) { return $node }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $timeout)
        throw "Missing accessible element '$name'"
    }
    [void](Find-ByName 'Pause')
    $control = Find-ByName 'Frame interpolation'
    if ($control.Current.IsEnabled) { throw 'Invalid provider enabled interpolation' }
    (Find-ByName 'Enhancements').GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    [void](Find-ByName $ExpectedWarning)
    'The optional provider was disabled with the expected integrity warning.'
    (Find-ByName 'Back to player').GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    python "$PSScriptRoot/visible_playback.py" --pid $process.Id
    if ($LASTEXITCODE) { throw "Baseline playback did not show moving colors" }
    'Baseline rendered, interpolation was disabled and the provider warning was visible.'
} finally {
    if (!$process.HasExited) {
        [void]$process.CloseMainWindow()
        if (!$process.WaitForExit(7000)) {
            $process.Kill()
            Write-Warning 'Test player shutdown timed out'
        }
    }
}
