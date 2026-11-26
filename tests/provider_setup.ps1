param([Parameter(Mandatory)][string]$Player,[Parameter(Mandatory)][string]$Fixture)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
$exe=(Resolve-Path $Player).Path
$folder=Split-Path -Parent $exe
if(!(Test-Path -LiteralPath (Join-Path $folder 'portable.mode')) -or (Test-Path -LiteralPath (Join-Path $folder 'data/library.sqlite3'))){throw 'Use a fresh portable test build'}
$media=(Resolve-Path $Fixture).Path
function Control([string]$Name){
    $deadline=[datetime]::UtcNow.AddSeconds(120)
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
function Launch {
    $script:app=Start-Process $exe -ArgumentList ('"'+$media+'"') -PassThru
    Click 'Settings'
}
try{
    Launch
    Click 'Install RIFE for playback';Click 'Cancel setup'
    [void](Control 'Setup cancelled.')
    if(Test-Path "$folder/providers/rife"){throw 'Cancelled installation left a provider'}
    Click 'Install RIFE for playback'
    [void](Control 'Installed and verified. Restart the player to use the provider.')
    if(!(Test-Path -LiteralPath "$folder/providers/rife/librife.dll")){throw 'Missing provider'}
    Close-Test
    Launch
    Click 'Install BestSource for export'
    [void](Control 'Installed and verified. Restart the player to use the provider.')
    if(!(Test-Path -LiteralPath "$folder/providers/rife-export/bestsource.dll")){throw 'Missing reader'}
    Close-Test
    Launch
    Click 'Back to player'
    Click 'Enhancements'
    (Control 'RIFE 2×').GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select()
    Start-Sleep -Seconds 1
    Click 'Back to player'
    python "$PSScriptRoot/visible_playback.py" --pid $app.Id
    if($LASTEXITCODE){throw 'Missing visible motion'}
    'Cancelled partial setup, installed and verified both providers, restarted and played visible RIFE output.'
}finally{Close-Test}