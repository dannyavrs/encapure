# Encapure - Download Models from GitHub Release
# Run this after cloning the repository to fetch the pre-quantized ONNX models.

param(
    [string]$Version = "latest"
)

$ErrorActionPreference = "Stop"
$RepoOwner = "dannyavrs"
$RepoName = "encapure"

Write-Host ""
Write-Host "==========================================================================" -ForegroundColor Cyan
Write-Host "  Encapure - Model Downloader" -ForegroundColor Cyan
Write-Host "==========================================================================" -ForegroundColor Cyan
Write-Host ""

# Resolve release URL
if ($Version -eq "latest") {
    $ReleaseUrl = "https://api.github.com/repos/$RepoOwner/$RepoName/releases/latest"
} else {
    $ReleaseUrl = "https://api.github.com/repos/$RepoOwner/$RepoName/releases/tags/$Version"
}

Write-Host "Fetching release info... " -NoNewline
try {
    $Release = Invoke-RestMethod -Uri $ReleaseUrl -Headers @{ "User-Agent" = "encapure-downloader" }
    Write-Host "OK ($($Release.tag_name))" -ForegroundColor Green
} catch {
    Write-Host "FAILED" -ForegroundColor Red
    Write-Host ""
    Write-Host "Could not fetch release from GitHub." -ForegroundColor Yellow
    Write-Host "Make sure the release exists at:" -ForegroundColor Yellow
    Write-Host "  https://github.com/$RepoOwner/$RepoName/releases" -ForegroundColor Gray
    Write-Host ""
    Write-Host "You can also download models manually:" -ForegroundColor Yellow
    Write-Host "  1. Go to the Releases page on GitHub" -ForegroundColor Gray
    Write-Host "  2. Download 'models.tar.gz'" -ForegroundColor Gray
    Write-Host "  3. Extract it in the project root: tar -xzf models.tar.gz" -ForegroundColor Gray
    exit 1
}

# Find the models archive asset
$Asset = $Release.assets | Where-Object { $_.name -eq "models.tar.gz" }

if (-not $Asset) {
    Write-Host "ERROR: No 'models.tar.gz' asset found in release $($Release.tag_name)" -ForegroundColor Red
    Write-Host "Available assets:" -ForegroundColor Yellow
    $Release.assets | ForEach-Object { Write-Host "  - $($_.name)" -ForegroundColor Gray }
    exit 1
}

$DownloadUrl = $Asset.browser_download_url
$OutputFile = "models.tar.gz"
$SizeMB = [math]::Round($Asset.size / 1MB, 1)

Write-Host "Downloading models ($SizeMB MB)... " -NoNewline

try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $OutputFile -Headers @{ "User-Agent" = "encapure-downloader" }
    Write-Host "OK" -ForegroundColor Green
} catch {
    Write-Host "FAILED" -ForegroundColor Red
    Write-Host "  $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}

# Extract
Write-Host "Extracting models... " -NoNewline
tar -xzf $OutputFile
Write-Host "OK" -ForegroundColor Green

# Cleanup
Remove-Item $OutputFile -Force

# Verify
$RequiredFiles = @(
    "models/model_int8.onnx",
    "models/tokenizer.json",
    "bi-encoder-model/model_int8.onnx",
    "bi-encoder-model/tokenizerbiencoder.json"
)

Write-Host ""
Write-Host "Verifying model files:" -ForegroundColor White

$AllPresent = $true
foreach ($File in $RequiredFiles) {
    if (Test-Path $File) {
        $Size = [math]::Round((Get-Item $File).Length / 1MB, 1)
        Write-Host "  [OK] $File ($Size MB)" -ForegroundColor Green
    } else {
        Write-Host "  [MISSING] $File" -ForegroundColor Red
        $AllPresent = $false
    }
}

Write-Host ""
if ($AllPresent) {
    Write-Host "All models downloaded successfully." -ForegroundColor Green
    Write-Host "You can now start the server:" -ForegroundColor White
    Write-Host ""
    Write-Host '  $env:ENCAPURE_MODE = "single"' -ForegroundColor Gray
    Write-Host '  $env:TOOLS_PATH = "tests/data/comprehensive_mock_tools.json"' -ForegroundColor Gray
    Write-Host '  .\target\release\encapure.exe' -ForegroundColor Gray
} else {
    Write-Host "Some model files are missing. Check the release archive." -ForegroundColor Red
    exit 1
}

Write-Host ""
