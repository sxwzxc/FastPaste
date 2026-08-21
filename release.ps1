param(
    [string]$Version,
    [switch]$SkipTest,
    [switch]$SkipBuild,
    [switch]$NoPause
)

# FastPaste 一键 Release 脚本
# 功能: 版本 bump -> cargo test -> cargo build --release -> 更新 README/CHANGELOG -> 复制到 dist + sha256
# 用法:
#   .\release.ps1                         # 交互式，保持当前版本
#   .\release.ps1 -Version 0.2.0           # bump 到 0.2.0 再编译
#   .\release.ps1 -SkipTest               # 跳过测试
#   .\release.ps1 -SkipBuild              # 仅更新文档，不编译（调试用）
#   release.bat 0.2.0 -SkipTest           # bat 入口同样支持

$ErrorActionPreference = "Stop"
$PSDefaultParameterValues['*:Encoding'] = 'utf8'

function Write-Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "[OK] $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Write-Err($msg)  { Write-Host "[ERR] $msg" -ForegroundColor Red }
function Set-ContentNoBom($Path, $Value) {
    # PowerShell 5.1 Set-Content -Encoding UTF8 会带 BOM，用 .NET 写入无 BOM 的 UTF8 以保持仓库文件干净
    [System.IO.File]::WriteAllText($Path, $Value, (New-Object System.Text.UTF8Encoding $false))
}

# 兼容 bat 传入的首个位置参数为版本号（release.bat 0.2.0）
# 当 -Version 未显式传值但 $args[0] 像版本号时，自动识别
if (-not $Version -and $args.Count -gt 0) {
    foreach ($a in $args) {
        if ($a -match '^\d+\.\d+\.\d+$') { $Version = $a; break }
        if ($a -eq '-SkipTest') { $SkipTest = $true }
        if ($a -eq '-SkipBuild') { $SkipBuild = $true }
        if ($a -eq '-NoPause') { $NoPause = $true }
    }
}

# 定位项目根目录（脚本所在目录）
$Root = if ($PSScriptRoot) { $PSScriptRoot } else { (Get-Location).Path }
if (-not (Test-Path -LiteralPath (Join-Path $Root "Cargo.toml"))) {
    # 尝试以调用路径为根
    $Root = (Get-Location).Path
}
Set-Location -LiteralPath $Root
Write-Host "Working dir: $Root" -ForegroundColor DarkGray

# 前置检查
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Err "未找到 cargo，请先安装 Rust: https://www.rust-lang.org/tools/install"
    if (-not $NoPause) { pause }
    exit 1
}
foreach ($f in @("Cargo.toml", "app.manifest", "README.md", "CHANGELOG.md")) {
    if (-not (Test-Path -LiteralPath (Join-Path $Root $f))) {
        Write-Err "缺少必要文件: $f"
        if (-not $NoPause) { pause }
        exit 1
    }
}

# 读取当前版本
$cargoToml = Get-Content -LiteralPath (Join-Path $Root "Cargo.toml") -Raw -Encoding UTF8
if ($cargoToml -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    Write-Err "无法从 Cargo.toml 解析 version"
    if (-not $NoPause) { pause }
    exit 1
}
$CurrentVersion = $Matches[1]
Write-Host "当前版本: $CurrentVersion" -ForegroundColor DarkGray

# 确定目标版本
$TargetVersion = $Version
if (-not $TargetVersion) {
    # 交互式询问（双击场景）
    try {
        $inputVer = Read-Host "输入新版本 (如 0.2.0，直接回车保持 $CurrentVersion)"
        $inputVer = $inputVer.Trim()
        if ([string]::IsNullOrWhiteSpace($inputVer)) {
            $TargetVersion = $CurrentVersion
        } else {
            $TargetVersion = $inputVer
        }
    } catch {
        $TargetVersion = $CurrentVersion
    }
}
$TargetVersion = $TargetVersion.Trim()

if ($TargetVersion -notmatch '^\d+\.\d+\.\d+$') {
    Write-Err "版本号格式非法: '$TargetVersion'，应为 x.y.z (如 0.2.0)"
    if (-not $NoPause) { pause }
    exit 1
}

$IsBump = ($TargetVersion -ne $CurrentVersion)
if ($IsBump) {
    Write-Step "版本 bump: $CurrentVersion -> $TargetVersion"
} else {
    Write-Step "保持当前版本: $TargetVersion"
}

# 更新 Cargo.toml
if ($IsBump) {
    # 仅替换 [package] 下的第一个 version
    # 策略：按行处理，找到 [package] 后首个 version 行替换
    $lines = Get-Content -LiteralPath (Join-Path $Root "Cargo.toml") -Encoding UTF8
    $inPackage = $false
    $replaced = $false
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $trim = $lines[$i].Trim()
        if ($trim -match '^\[package\]') { $inPackage = $true; continue }
        if ($inPackage -and $trim -match '^\[') { $inPackage = $false }
        if ($inPackage -and -not $replaced -and $lines[$i] -match '^\s*version\s*=\s*".*?"') {
            $lines[$i] = $lines[$i] -replace 'version\s*=\s*".*?"', "version = `"$TargetVersion`""
            $replaced = $true
            break
        }
    }
    if (-not $replaced) {
        # 回退：全局替换第一个
        $cargoToml = $cargoToml -replace 'version\s*=\s*".*?"', "version = `"$TargetVersion`"", 1
        Set-ContentNoBom -Path (Join-Path $Root "Cargo.toml") -Value $cargoToml
    } else {
        Set-ContentNoBom -Path (Join-Path $Root "Cargo.toml") -Value ($lines -join "`r`n")
    }
    Write-Ok "已更新 Cargo.toml -> $TargetVersion"

    # 更新 app.manifest 的 assemblyIdentity version="x.y.z.0"
    $manifestPath = Join-Path $Root "app.manifest"
    $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8
    $manifestVer4 = "$TargetVersion.0"
    if ($manifest -match 'assemblyIdentity[^>]*version="[^"]+"') {
        $manifest = $manifest -replace '(assemblyIdentity[^>]*version=")[^"]+(")', "`${1}$manifestVer4`$2"
        Set-ContentNoBom -Path $manifestPath -Value $manifest
        Write-Ok "已更新 app.manifest -> $manifestVer4"
    } else {
        Write-Warn "app.manifest 未找到 version 属性，跳过"
    }
}

# cargo test
if (-not $SkipTest) {
    Write-Step "cargo test"
    cargo test
    if ($LASTEXITCODE -ne 0) {
        Write-Err "cargo test 失败 (exit $LASTEXITCODE)，已终止。请修复后再发布。"
        Write-Host "如需跳过测试: .\release.ps1 -SkipTest" -ForegroundColor DarkGray
        if (-not $NoPause) { pause }
        exit $LASTEXITCODE
    }
    Write-Ok "cargo test 通过"
} else {
    Write-Warn "已跳过 cargo test (-SkipTest)"
}

# cargo build --release
if (-not $SkipBuild) {
    Write-Step "cargo build --release  (已启用 lto=true，耗时较长...)"
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Err "cargo build --release 失败 (exit $LASTEXITCODE)"
        if (-not $NoPause) { pause }
        exit $LASTEXITCODE
    }
    Write-Ok "cargo build --release 完成"
} else {
    Write-Warn "已跳过 cargo build (-SkipBuild)"
}

# 更新 README.md 的 release-info 块
Write-Step "更新 README.md"
$readmePath = Join-Path $Root "README.md"
$readme = Get-Content -LiteralPath $readmePath -Raw -Encoding UTF8
$buildTime = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
$commitHash = "unknown"
try {
    $commitHash = (git rev-parse --short HEAD 2>$null).Trim()
    if (-not $commitHash) { $commitHash = "unknown" }
} catch { $commitHash = "unknown" }

$newInfo = "<!-- release-info:start -->`r`n> **当前版本**: ``v$TargetVersion`` | **构建时间**: $buildTime | **Commit**: ``$commitHash```r`n<!-- release-info:end -->"

if ($readme -match '<!-- release-info:start -->[\s\S]*?<!-- release-info:end -->') {
    $readme = $readme -replace '<!-- release-info:start -->[\s\S]*?<!-- release-info:end -->', $newInfo
    Set-ContentNoBom -Path $readmePath -Value $readme
    Write-Ok "已更新 README.md release-info -> v$TargetVersion $buildTime $commitHash"
} else {
    Write-Warn "README.md 未找到 <!-- release-info --> 标记，已跳过（请手动添加）"
}

# 更新 CHANGELOG.md
Write-Step "更新 CHANGELOG.md"
$changelogPath = Join-Path $Root "CHANGELOG.md"
$changelog = Get-Content -LiteralPath $changelogPath -Raw -Encoding UTF8
$today = Get-Date -Format "yyyy-MM-dd"

if ($changelog -match "## \[$([regex]::Escape($TargetVersion))\]") {
    # 已存在该版本条目，仅更新日期
    $escaped = [regex]::Escape($TargetVersion)
    $changelog = $changelog -replace "## \[$escaped\] - \d{4}-\d{2}-\d{2}", "## [$TargetVersion] - $today"
    Set-ContentNoBom -Path $changelogPath -Value $changelog
    Write-Ok "已更新 CHANGELOG.md 日期 -> $today"
} elseif ($IsBump) {
    # 新增版本条目：插入到第一个 ## [x.y.z] 之前
    $newEntry = "## [$TargetVersion] - $today`r`n`r`n### Changed`r`n- ...`r`n`r`n"
    if ($changelog -match '(?m)^## \[\d+\.\d+\.\d+\]') {
        $changelog = $changelog -replace '(?m)^(## \[\d+\.\d+\.\d+\])', "$newEntry`$1"
    } else {
        # 无既有条目，追加到末尾
        $changelog = $changelog.TrimEnd() + "`r`n`r`n" + $newEntry
    }
    Set-ContentNoBom -Path $changelogPath -Value $changelog
    Write-Ok "已新增 CHANGELOG.md 条目 -> [$TargetVersion] - $today"
} else {
    Write-Host "CHANGELOG.md 保持不变（未 bump 版本）" -ForegroundColor DarkGray
}

# 复制产物到 dist + 生成 sha256
if (-not $SkipBuild) {
    Write-Step "生成 dist 产物"
    $exeName = "fastpaste.exe"
    $srcExe = Join-Path $Root "target\release\$exeName"
    # 兼容部分环境生成 fastpaste 而非 fastpaste.exe
    if (-not (Test-Path -LiteralPath $srcExe)) {
        $alt = Join-Path $Root "target\release\fastpaste"
        if (Test-Path -LiteralPath $alt) { $srcExe = $alt }
    }
    if (-not (Test-Path -LiteralPath $srcExe)) {
        Write-Err "未找到编译产物: $srcExe"
        Write-Host "请检查 cargo build --release 是否成功，或手动查看 target\release\" -ForegroundColor DarkGray
        if (-not $NoPause) { pause }
        exit 1
    }

    $distDir = Join-Path $Root "dist"
    if (-not (Test-Path -LiteralPath $distDir)) {
        New-Item -ItemType Directory -Path $distDir | Out-Null
    }
    $dstExe = Join-Path $distDir $exeName
    Copy-Item -LiteralPath $srcExe -Destination $dstExe -Force
    $sizeMB = "{0:N2}" -f ((Get-Item -LiteralPath $dstExe).Length / 1MB)
    $sizeKB = "{0:N0}" -f ((Get-Item -LiteralPath $dstExe).Length / 1KB)
    Write-Ok "已复制 $srcExe -> $dstExe ($sizeMB MB / $sizeKB KB)"

    # sha256
    $hash = (Get-FileHash -LiteralPath $dstExe -Algorithm SHA256).Hash.ToLower()
    $shaFile = "$dstExe.sha256"
    # 生成类似 sha256sum 的格式： "<hash>  fastpaste.exe"
    Set-ContentNoBom -Path $shaFile -Value "$hash  $exeName"
    Write-Ok "已生成 $shaFile"
    Write-Host "  $hash  $exeName" -ForegroundColor DarkGray

    # 可选：显示 dist 目录
    Write-Host ""
    Get-ChildItem -LiteralPath $distDir | Format-Table Name, Length, LastWriteTime -AutoSize | Out-String | Write-Host
}

# 总结与 git 提示
Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host " Release 完成: v$TargetVersion" -ForegroundColor Green
Write-Host " 构建时间: $buildTime" -ForegroundColor DarkGray
Write-Host " Commit:   $commitHash" -ForegroundColor DarkGray
if (-not $SkipBuild -and (Test-Path -LiteralPath (Join-Path $Root "dist\fastpaste.exe"))) {
    $finalSize = (Get-Item -LiteralPath (Join-Path $Root "dist\fastpaste.exe")).Length
    $finalMB = "{0:N2}" -f ($finalSize / 1MB)
    Write-Host " 产物:     dist\fastpaste.exe ($finalMB MB)" -ForegroundColor Cyan
    Write-Host " 校验:     dist\fastpaste.exe.sha256" -ForegroundColor Cyan
}
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "下一步 (git 发布):" -ForegroundColor Yellow
Write-Host "  git add Cargo.toml app.manifest README.md CHANGELOG.md" -ForegroundColor White
Write-Host "  git commit -m `"chore(release): v$TargetVersion`"" -ForegroundColor White
Write-Host "  git tag v$TargetVersion" -ForegroundColor White
Write-Host "  git push && git push --tags" -ForegroundColor White
Write-Host ""
if ($IsBump) {
    Write-Host "提示: 已 bump 版本，请检查 Cargo.toml / app.manifest / README / CHANGELOG 是否符合预期后再提交。" -ForegroundColor DarkGray
}

if (-not $NoPause -and [Environment]::UserInteractive) {
    # 双击场景下，bat 已有 pause，此处仅在直接双击 ps1 时生效
    # 通过检测父进程是否为 explorer 来避免重复暂停（尽力而为）
    $needPause = $true
    try {
        $parent = (Get-CimInstance Win32_Process -Filter "ProcessId=$PID" -ErrorAction SilentlyContinue).ParentProcessId
        if ($parent) {
            $pName = (Get-Process -Id $parent -ErrorAction SilentlyContinue).ProcessName
            if ($pName -eq "cmd" -or $pName -eq "powershell" -or $pName -eq "pwsh") {
                # 由 bat/cmd 调起，bat 会负责 pause，脚本不再二次暂停
                $needPause = $false
            }
        }
    } catch { $needPause = $true }

    if ($needPause) {
        Write-Host ""
        Write-Host "按回车键退出..." -ForegroundColor DarkGray
        [void](Read-Host)
    }
}
