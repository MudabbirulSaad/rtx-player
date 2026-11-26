param([Parameter(Mandatory)][string]$Folder)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
$exe=Join-Path $Folder 'rtx-player-desktop.exe'
if(!(Test-Path "$Folder/portable.mode")){throw 'Portable test build required'}
# Use a staged portable copy and two synthetic files named Pattern A.mp4 and Pattern B.mkv.
$Folder=(Resolve-Path -LiteralPath $Folder).Path
$Media=(Resolve-Path -LiteralPath (Join-Path $Folder 'Pattern A.mp4')).Path
[void](Resolve-Path -LiteralPath (Join-Path $Folder 'Pattern B.mkv'))
$profile=Join-Path $Folder 'data'
if(Test-Path -LiteralPath $profile){throw 'Use a fresh portable test folder with no data directory'}
New-Item -ItemType Directory -Path $profile -ErrorAction Stop|Out-Null
$fixture=Join-Path $profile 'library.sqlite3'
[IO.File]::WriteAllText($fixture,'Corrupt synthetic database for recovery verification')
$before=(Get-FileHash -LiteralPath $fixture).Hash
function Find-Item($name){
  $r=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
  $r.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$name))
}
function Wait-Item($name){
  $stop=[datetime]::UtcNow.AddSeconds(12)
  do{$app.Refresh();if($app.MainWindowHandle -and $app.MainWindowTitle -eq 'RTX Player'){$o=Find-Item $name;if($o -and $o.Current.IsEnabled){return $o}};Start-Sleep -Milliseconds 80}while(!$app.HasExited -and [datetime]::UtcNow -lt $stop)
  throw "Did not see $name"
}
function Click($name){$o=Wait-Item $name;$p=$null;if($o.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$p)){$p.Invoke()}elseif($o.TryGetCurrentPattern([Windows.Automation.TogglePattern]::Pattern,[ref]$p)){$p.Toggle()}else{throw "Missing click pattern: $name"}}
function Close-App{if(!$app.HasExited){[void]$app.CloseMainWindow();if(!$app.WaitForExit(8000)){$app.Kill();throw 'Test application stalled'}}}
$app=Start-Process $exe -ArgumentList ('"'+$Media+'"') -PassThru
try{
    Click 'Settings'
    [void](Wait-Item 'Your original library database is damaged and has been preserved. New settings and library entries use library-recovery.sqlite3.')
    Click 'Back to player'
    python "$PSScriptRoot/visible_playback.py" --pid $app.Id
    if($LASTEXITCODE){throw 'No video with corrupt library'}
}finally{Close-App}
$db=Join-Path $Folder 'data/library-recovery.sqlite3'
python -c "import sqlite3,sys,pathlib; db=sqlite3.connect(sys.argv[1]); c=db.execute('insert into collections(name) values(?)',('Seek check',)).lastrowid;db.executemany('insert into members(collection_id,ordinal,path,title,bytes) values(?,?,?,?,?)',[(c,i,str(p),p.name,p.stat().st_size) for i,p in enumerate(map(pathlib.Path,sys.argv[2:]))]);db.commit()" $db (Join-Path $Folder 'Pattern A.mp4') (Join-Path $Folder 'Pattern B.mkv')
if($LASTEXITCODE){throw 'Fixture setup failed'}
$app=Start-Process $exe -PassThru
try{
    Click 'Library';Click 'Folders and playlists';Click 'Seek check 2 videos';Click 'Play all'
    [void](Wait-Item 'Pattern A.mp4')
    Click 'Pause'
    $samples=@()
    foreach($i in 0..47){
        $which=if($i%2 -eq 0){'Next in playlist'}else{'Previous in playlist'}
        $title=if($i%2 -eq 0){'Pattern B.mkv'}else{'Pattern A.mp4'}
        Click $which;[void](Wait-Item $title)
        $seek=(Wait-Item 'Seek').GetCurrentPattern([Windows.Automation.RangeValuePattern]::Pattern)
        $t=2+($i%8)
        $seek.SetValue($t)
        Start-Sleep -Milliseconds 350
        $deadline=[datetime]::UtcNow.AddSeconds(2)
        while([math]::Abs($seek.Current.Value-$t) -gt 0.5 -and [datetime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 65}
        if([math]::Abs($seek.Current.Value-$t) -gt 1.4){throw "Seek did not settle near $t on $title actual=$($seek.Current.Value) cycle=$i"}
        $app.Refresh();$samples+= [math]::Round($app.PrivateMemorySize64/1MB,1)
    }
    if(Find-Item 'Play'){Click 'Play'}
    python "$PSScriptRoot/visible_playback.py" --pid $app.Id
    if($LASTEXITCODE){throw 'No moving frames after repeated switches and seeks'}
    "48 queue switches and seeks, host private MiB first=$($samples[0]) warm=$($samples[11]) last=$($samples[-1]) peak=$(($samples|Measure-Object -Maximum).Maximum)"
}finally{Close-App}
if((Get-FileHash -LiteralPath $fixture).Hash -ne $before){throw 'Original database changed'}
