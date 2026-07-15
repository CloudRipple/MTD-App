$ErrorActionPreference = "Stop"
$FontFile = "assets\fonts\HarmonyOS_Sans_SC_Regular.ttf"

if (!(Test-Path $FontFile)) {
  Write-Error "Missing $FontFile. Download HarmonyOS Sans from the official Huawei design resource page, then place the Simplified Chinese regular TTF at this path."
}

.\scripts\build-ffmpeg-windows.ps1

$FfmpegFiles = @(
  "vendor\ffmpeg\windows\ffmpeg.exe",
  "vendor\ffmpeg\windows\BUILD_INFO.txt"
)

function Get-Msys2Root {
  if (![string]::IsNullOrWhiteSpace($env:MSYS2_BASH)) {
    $BashPath = (Resolve-Path $env:MSYS2_BASH).Path
    $UsrBin = Split-Path $BashPath -Parent
    $Usr = Split-Path $UsrBin -Parent
    return Split-Path $Usr -Parent
  }

  $Candidates = @(
    "C:\msys64",
    "C:\msys64\mingw64\.."
  )
  foreach ($Candidate in $Candidates) {
    if (Test-Path $Candidate) {
      return (Resolve-Path $Candidate).Path
    }
  }

  return $null
}

function Get-ObjdumpPath($MingwBin) {
  $Candidates = @(
    (Join-Path $MingwBin "objdump.exe"),
    "C:\msys64\mingw64\bin\objdump.exe",
    "C:\msys64\usr\bin\objdump.exe"
  )
  foreach ($Candidate in $Candidates) {
    if (Test-Path $Candidate) {
      return (Resolve-Path $Candidate).Path
    }
  }

  return $null
}

function Test-SystemDll($Name) {
  $Lower = $Name.ToLowerInvariant()
  if ($Lower -match "^(api-ms-|ext-ms-)") {
    return $true
  }

  $SystemDlls = @(
    "advapi32.dll",
    "avicap32.dll",
    "bcrypt.dll",
    "cfgmgr32.dll",
    "comdlg32.dll",
    "comctl32.dll",
    "crypt32.dll",
    "dwrite.dll",
    "dwmapi.dll",
    "gdi32.dll",
    "imm32.dll",
    "kernel32.dll",
    "msvcrt.dll",
    "ntdll.dll",
    "ole32.dll",
    "oleaut32.dll",
    "rpcrt4.dll",
    "secur32.dll",
    "setupapi.dll",
    "shell32.dll",
    "shlwapi.dll",
    "user32.dll",
    "version.dll",
    "winmm.dll",
    "winspool.drv",
    "ws2_32.dll"
  )

  return $SystemDlls -contains $Lower
}

function Get-DllDependencies($Objdump, $Binary) {
  $Output = & $Objdump -p $Binary 2>$null
  foreach ($Line in $Output) {
    if ($Line -match "DLL Name:\s*(.+)$") {
      $Matches[1].Trim()
    }
  }
}

function Copy-MingwRuntimeDlls($EntryBinary, $Destination) {
  $Msys2Root = Get-Msys2Root
  if ([string]::IsNullOrWhiteSpace($Msys2Root)) {
    Write-Error "MSYS2 root is required to bundle FFmpeg runtime DLLs."
  }

  $MingwBin = Join-Path $Msys2Root "mingw64\bin"
  if (!(Test-Path $MingwBin)) {
    Write-Error "Missing MSYS2 MinGW bin directory: $MingwBin"
  }

  $Objdump = Get-ObjdumpPath $MingwBin
  if ([string]::IsNullOrWhiteSpace($Objdump)) {
    Write-Error "objdump.exe is required to bundle FFmpeg runtime DLLs."
  }

  $Queue = New-Object System.Collections.Generic.Queue[string]
  $Visited = @{}
  $Queue.Enqueue((Resolve-Path $EntryBinary).Path)

  while ($Queue.Count -gt 0) {
    $Binary = $Queue.Dequeue()
    $BinaryKey = $Binary.ToLowerInvariant()
    if ($Visited.ContainsKey($BinaryKey)) {
      continue
    }
    $Visited[$BinaryKey] = $true

    foreach ($DllName in Get-DllDependencies $Objdump $Binary) {
      if (Test-SystemDll $DllName) {
        continue
      }

      $Source = Join-Path $MingwBin $DllName
      if (!(Test-Path $Source)) {
        $SystemSource = Join-Path $env:WINDIR "System32\$DllName"
        if (Test-Path $SystemSource) {
          continue
        }
        Write-Error "Missing runtime DLL for bundled FFmpeg: $DllName"
      }

      $DestinationDll = Join-Path $Destination $DllName
      Copy-Item $Source $DestinationDll -Force
      $Queue.Enqueue((Resolve-Path $Source).Path)
    }
  }
}

function Reset-Directory($Path) {
  if (Test-Path $Path) {
    Remove-Item -LiteralPath $Path -Recurse -Force
  }
  New-Item -ItemType Directory -Force -Path $Path | Out-Null
}

foreach ($FfmpegFile in $FfmpegFiles) {
  if (!(Test-Path $FfmpegFile)) {
    Write-Error "Missing $FfmpegFile after building bundled FFmpeg."
  }
}

$EmbeddedFfmpegDir = "build\embedded\windows\ffmpeg"
Reset-Directory $EmbeddedFfmpegDir
Copy-Item vendor\ffmpeg\windows\ffmpeg.exe $EmbeddedFfmpegDir\
Copy-MingwRuntimeDlls "$EmbeddedFfmpegDir\ffmpeg.exe" $EmbeddedFfmpegDir

$env:MTD_EMBED_FFMPEG_DIR = (Resolve-Path $EmbeddedFfmpegDir).Path
$env:MTD_EMBED_UI_FONT = (Resolve-Path $FontFile).Path
cargo build --release
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

Reset-Directory "dist\windows"
Copy-Item target\release\mtd-subtitle-app.exe dist\windows\MTDSubtitleApp.exe
Write-Host "Build output: dist\windows\MTDSubtitleApp.exe"
