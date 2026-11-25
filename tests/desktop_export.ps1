param([Parameter(Mandatory)][string]$Player,[Parameter(Mandatory)][string]$Fixture)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -Namespace ExportTest -Name Native -MemberDefinition @'
    [System.Runtime.InteropServices.DllImport("user32.dll")]
    public static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
    [System.Runtime.InteropServices.DllImport("user32.dll")]
    public static extern IntPtr GetDlgItem(IntPtr window, int id);
'@
$inputFile=(Resolve-Path -LiteralPath $Fixture).Path
$destinationPath=Join-Path (Split-Path -Parent $inputFile) ([IO.Path]::GetFileNameWithoutExtension($inputFile) + '-export.mp4')
if(Test-Path -LiteralPath $destinationPath){throw 'Test output must be a new file'}
$before=(Get-FileHash -LiteralPath $inputFile).Hash
$app=Start-Process -FilePath (Resolve-Path $Player) -ArgumentList ('"'+$inputFile+'"') -PassThru
function Named($root,$name) {
    $query=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$name)
    $root.FindFirst([Windows.Automation.TreeScope]::Descendants,$query)
}
function Wait-Value([scriptblock]$Read,[string]$Missing,[int]$Seconds=12){
    $end=[datetime]::UtcNow.AddSeconds($Seconds)
    do{
        if($app.HasExited){throw 'Player stopped'}
        $value=& $Read
        if($value){return $value}
        Start-Sleep -Milliseconds 150
    }while([datetime]::UtcNow -lt $end)
    throw $Missing
}
function Click($button){$button.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()}
try{
    $hwnd=Wait-Value {$app.Refresh();if($app.MainWindowTitle -eq 'RTX Player'){$app.MainWindowHandle}} 'Player window missing'
    $root=[Windows.Automation.AutomationElement]::FromHandle($hwnd)
    $export=Wait-Value {$b=Named $root 'Export';if($b -and $b.Current.IsEnabled){$b}} 'Export unavailable'
    Click $export
    Click (Wait-Value {Named $root 'Save as…'} 'Export panel did not open')
    $desktop=[Windows.Automation.AutomationElement]::RootElement
    $owner=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty,$app.Id)
    $title=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Export video')
    $windowType=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Window)
    $dialogMatch=[Windows.Automation.AndCondition]::new([Windows.Automation.Condition[]]@($owner,$title,$windowType))
    $dialog=Wait-Value {
        $result=$desktop.FindFirst([Windows.Automation.TreeScope]::Children,$dialogMatch)
        if(!$result){$result=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,$dialogMatch)}
        $result
    } 'Export save dialog missing'
    $save=[ExportTest.Native]::GetDlgItem($dialog.Current.NativeWindowHandle,1)
    if(!$save -or ![ExportTest.Native]::PostMessage($save,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'Save button unavailable'}
    [void](Wait-Value {Named $root 'Export saved'} 'Export did not finish' 45)
    if(!(Test-Path -LiteralPath $destinationPath -PathType Leaf)){throw 'No saved video'}
    if((Get-FileHash -LiteralPath $inputFile).Hash -ne $before){throw 'Source file changed'}
    Get-Item -LiteralPath $destinationPath |Select-Object Name,Length
}finally{
    if(!$app.HasExited){
        [void]$app.CloseMainWindow()
        if(!$app.WaitForExit(6000)){$app.Kill();throw 'Player did not close normally'}
    }
}
