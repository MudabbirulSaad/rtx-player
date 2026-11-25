param([Parameter(Mandatory)][string]$Player,[Parameter(Mandatory)][string]$Fixture)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
$exe=(Resolve-Path $Player).Path
$folder=Split-Path -Parent $exe
if(!(Test-Path -LiteralPath (Join-Path $folder 'portable.mode')) -or (Test-Path -LiteralPath (Join-Path $folder 'data/library.sqlite3'))){throw 'Use a fresh portable test build'}
$media=(Resolve-Path $Fixture).Path
function Control([string]$Name){
    $deadline=[datetime]::UtcNow.AddSeconds(10)
    do{
        $app.Refresh()
        if($app.HasExited){throw 'Player stopped'}
        if($app.MainWindowHandle -and $app.MainWindowTitle -eq 'RTX Player'){
            $root=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
            $query=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name)
            $result=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,$query)
            if($result -and $result.Current.IsEnabled){return $result}
        }
        Start-Sleep -Milliseconds 100
    }while([datetime]::UtcNow -lt $deadline)
    throw "Missing $Name"
}
function Click([string]$name){(Control $name).GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()}
function Close-Test {
    if($app -and !$app.HasExited){[void]$app.CloseMainWindow();if(!$app.WaitForExit(6000)){$app.Kill();throw 'Shutdown timeout'}}
}
try{
    $app=Start-Process $exe -ArgumentList ('"'+$media+'"') -PassThru
    Click 'Pause'
    Click 'Library'
    $history=(Control 'Remember recent videos').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
    if($history.Current.ToggleState -ne 'Off'){throw 'History was enabled in a fresh profile'}
    $history.Toggle()
    Start-Sleep -Milliseconds 400
    Click 'Back to player'
    $seek=(Control 'Seek').GetCurrentPattern([Windows.Automation.RangeValuePattern]::Pattern)
    $seek.SetValue(8)
    Start-Sleep -Milliseconds 600
    if([Math]::Abs($seek.Current.Value-8) -gt 0.25){throw 'Did not seek to the requested bookmark'}
    Close-Test
    $app=Start-Process $exe -PassThru
    Click 'Library'
    $title=[IO.Path]::GetFileName($media)
    $root=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
    $query=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Button)
    $stop=[datetime]::UtcNow.AddSeconds(8)
    do{
        $entry=$root.FindAll([Windows.Automation.TreeScope]::Descendants,$query)|Where-Object {$_.Current.Name -like "*$title*" }|Select-Object -First 1
        if(!$entry){Start-Sleep -Milliseconds 100}
    }while(!$entry -and [datetime]::UtcNow -lt $stop)
    if(!$entry){throw 'Missing recent entry'}
    $entry.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    $seek=(Control 'Seek').GetCurrentPattern([Windows.Automation.RangeValuePattern]::Pattern)
    Start-Sleep -Milliseconds 500
    $pos=$seek.Current.Value
    if($pos -lt 7.7 -or $pos -gt 11){throw "Wrong resume position $pos"}
    "History opt-in persisted. Reopened at $pos seconds."
} finally {Close-Test}
