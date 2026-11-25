param([Parameter(Mandatory)][string]$Player, [Parameter(Mandatory)][string]$Fixture)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
$app = Start-Process -FilePath (Resolve-Path $Player) -ArgumentList ('"' + (Resolve-Path $Fixture) + '"') -PassThru
function Find-Control([string]$Name) {
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    $match = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, $Name)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw "Player exited before $Name appeared" }
        if ($app.MainWindowHandle -ne 0) {
            $root = [Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
            $control = $root.FindFirst([Windows.Automation.TreeScope]::Descendants, $match)
            if ($control -and $control.Current.IsEnabled) { return $control }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Missing accessible control: $Name"
}
try {
    Start-Sleep -Milliseconds 1500
    $pause = Find-Control 'Pause'
    $pause.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    $play = Find-Control 'Play'
    $position = (Find-Control 'Seek').GetCurrentPattern([Windows.Automation.RangeValuePattern]::Pattern)
    Start-Sleep -Milliseconds 500
    $pausedAt = $position.Current.Value
    Start-Sleep -Milliseconds 700
    if ([Math]::Abs($position.Current.Value - $pausedAt) -gt 0.15) { throw 'Timeline advanced while paused' }
    $targetTime = 8.0
    $position.SetValue($targetTime)
    Start-Sleep -Milliseconds 700
    $play.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    [void](Find-Control 'Pause')
    Start-Sleep -Milliseconds 1000
    if ($position.Current.Value -le $targetTime + 0.3) { throw 'Timeline did not resume' }
    'Accessible pause, seek and resume changed playback time'
} finally {
    if (!$app.HasExited) {
        [void]$app.CloseMainWindow()
        if (!$app.WaitForExit(5000)) { $app.Kill(); throw 'Player did not close normally' }
    }
}
