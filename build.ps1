param(
    [switch]$Release,
    [switch]$Debug,
    [switch]$Run,
    [switch]$Test,
    [switch]$NoPause
)

# FastPaste 快捷编译脚本（双击即用）
# 用法:
#   .\build.ps1              # 默认 debug 快速编译 (cargo build)
#   .\build.ps1 -Release     # release 编译 (cargo build --release，含提权 manifest，耗时较长)
#   .\build.ps1 -Test        # 先 cargo test 再编译
#   .\build.ps1 -Run         # 编译后自动运行
#   .\build.ps1 -Release -Run
#   build.bat -Release       # bat 入口同样支持

$ErrorActionPreference = "Stop"

$Root = if ($PSScriptRoot) { $PSScriptRoot } else { (Get-Location).Path }
Set-Location -LiteralPath $Root
Write-Host "Working dir: $Root" -ForegroundColor DarkGray

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "[ERR] 未找到 cargo，请先安装 Rust" -ForegroundColor Red
    if (-not $NoPause) { pause }
    exit 1
}

# 兼容 bat 透传的参数
foreach ($a in $args) {
    if ($a -eq '-Release') { $Release = $true }
    if ($a -eq '-Debug') { $Debug = $true }
    if ($a -eq '-Run') { $Run = $true }
    if ($a -eq '-Test') { $Test = $true }
    if ($a -eq '-NoPause') { $NoPause = $true }
}

# 默认策略：无显式开关则为 Debug（快速）
$doRelease = $false
if ($Release) { $doRelease = $true }
elseif ($Debug) { $doRelease = $false }
else { $doRelease = $false }

if ($Test) {
    Write-Host "`n==> cargo test" -ForegroundColor Cyan
    cargo test
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERR] cargo test 失败 ($LASTEXITCODE)" -ForegroundColor Red
        if (-not $NoPause) { pause }
        exit $LASTEXITCODE
    }
    Write-Host "[OK] cargo test 通过" -ForegroundColor Green
}

if ($doRelease) {
    Write-Host "`n==> cargo build --release" -ForegroundColor Cyan
    Write-Host "提示: release 已启用 lto=true，首次编译耗时较长..." -ForegroundColor DarkGray
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERR] cargo build --release 失败 ($LASTEXITCODE)" -ForegroundColor Red
        if (-not $NoPause) { pause }
        exit $LASTEXITCODE
    }
    $exe = Join-Path $Root "target\release\fastpaste.exe"
    if (-not (Test-Path -LiteralPath $exe)) { $exe = Join-Path $Root "target\release\fastpaste" }
} else {
    Write-Host "`n==> cargo build (debug)" -ForegroundColor Cyan
    cargo build
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERR] cargo build 失败 ($LASTEXITCODE)" -ForegroundColor Red
        if (-not $NoPause) { pause }
        exit $LASTEXITCODE
    }
    $exe = Join-Path $Root "target\debug\fastpaste.exe"
    if (-not (Test-Path -LiteralPath $exe)) { $exe = Join-Path $Root "target\debug\fastpaste" }
}

if (Test-Path -LiteralPath $exe) {
    $size = (Get-Item -LiteralPath $exe).Length
    $mb = "{0:N2}" -f ($size / 1MB)
    Write-Host "`n[OK] 编译完成: $exe ($mb MB)" -ForegroundColor Green
} else {
    Write-Host "`n[OK] 编译完成（未找到预期产物路径，请查看 target\）" -ForegroundColor Green
}

if ($Run) {
    Write-Host "`n==> 运行 $exe" -ForegroundColor Cyan
    if (Test-Path -LiteralPath $exe) {
        & $exe
    } else {
        cargo run
    }
}

if (-not $NoPause) {
    # 若由 build.bat 调起，bat 会 pause，此处避免重复
    $needPause = $true
    try {
        $parent = (Get-CimInstance Win32_Process -Filter "ProcessId=$PID" -ErrorAction SilentlyContinue).ParentProcessId
        if ($parent) {
            $pName = (Get-Process -Id $parent -ErrorAction SilentlyContinue).ProcessName
            if ($pName -eq "cmd") { $needPause = $false }
        }
    } catch {}
    if ($needPause -and [Environment]::UserInteractive) {
        Write-Host ""
        Write-Host "按回车键退出..." -ForegroundColor DarkGray
        [void](Read-Host)
    }
}
