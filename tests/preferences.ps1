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
    Click 'Settings'
    $toggle=(Control 'Use software decoding').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
    if($toggle.Current.ToggleState -ne 'Off'){throw 'Wrong initial option'}
    $toggle.Toggle()
    Click 'Save preferences'
    [void](Control 'Preferences saved')
    Close-Test
    $app=Start-Process $exe -ArgumentList ('"'+$media+'"') -PassThru
    Click 'Settings'
    $checked=(Control 'Use software decoding').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Current.ToggleState
    if($checked -ne 'On'){throw 'Preference was lost'}
    Click 'Back to player'
    (Control 'Playback statistics').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Toggle()
    Start-Sleep -Milliseconds 600
    $root=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
    $actual=$root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.Name -like '*Filter *fps*'}|ForEach-Object {$_.Current.Name}
    $actual
    if(($actual -join ' ') -notmatch '\bno\b'){throw 'Software decoder not reported'}
    python "$PSScriptRoot/visible_playback.py" --pid $app.Id
    if($LASTEXITCODE){throw 'No visible motion'}
}finally{Close-Test}