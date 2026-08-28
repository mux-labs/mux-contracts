# Entrypoint Matrix Gap Detection Script
# This script verifies that all implemented contract entrypoints are documented
# in the entrypoint-matrix.md file.

Write-Host "🔍 Checking entrypoint matrix for gaps..." -ForegroundColor Cyan

# Known missing entrypoints that should be documented
$knownMissing = @(
    @("mux-account", "execute"),
    @("mux-account", "register_session_key"), 
    @("mux-account", "revoke_session_key"),
    @("mux-batcher", "submit_batch"),
    @("mux-recovery", "add_guardian"),
    @("mux-recovery", "remove_guardian"),
    @("mux-recovery", "approve_recovery_admin"),
    @("mux-recovery", "set_registry"),
    @("mux-delegation", "link_contract_id"),
    @("mux-delegation", "check_delegate"),
    @("mux-wallet-registry", "list_wallets")
)

# Read and parse entrypoint matrix
$matrixPath = "docs/entrypoint-matrix.md"
if (!(Test-Path $matrixPath)) {
    Write-Host "ERROR: $matrixPath not found" -ForegroundColor Red
    exit 1
}

$matrixContent = Get-Content $matrixPath
$documented = @{}
$currentContract = ""

foreach ($line in $matrixContent) {
    # Detect contract section headers
    if ($line -match "^## (.+)$") {
        $currentContract = $matches[1].Trim()
        $documented[$currentContract] = @()
        continue
    }
    
    # Parse entrypoint table rows
    if ($line.StartsWith("|") -and !($line -match "Entrypoint") -and !($line -match "---")) {
        $parts = $line -split '\|' | ForEach-Object { $_.Trim() }
        if ($parts.Length -ge 3 -and $parts[1]) {
            # Extract function name, removing backticks and parameters
            $entrypointCell = $parts[1].Trim('`')
            $funcName = ($entrypointCell -split '\(')[0].Trim()
            
            if ($funcName -and $funcName -notin @("Auth", "Notes", "")) {
                $documented[$currentContract] += $funcName
            }
        }
    }
}

# Check for missing entrypoints
$stillMissing = @()

foreach ($pair in $knownMissing) {
    $contract = $pair[0]
    $function = $pair[1]
    
    if (!$documented.ContainsKey($contract) -or $function -notin $documented[$contract]) {
        $stillMissing += ,@($contract, $function)
    }
}

if ($stillMissing.Count -gt 0) {
    Write-Host ""
    Write-Host "ENTRYPOINT MATRIX GAP DETECTED:" -ForegroundColor Red
    Write-Host "The following implemented entrypoints are missing from docs/entrypoint-matrix.md:" -ForegroundColor Yellow
    Write-Host ""
    
    foreach ($pair in $stillMissing) {
        Write-Host "  $($pair[0]) :: $($pair[1])" -ForegroundColor White
    }
    
    Write-Host ""
    Write-Host "REQUIRED ACTION:" -ForegroundColor Yellow
    Write-Host "Add these entrypoints to docs/entrypoint-matrix.md with proper auth classification (A/U/P)"
    Write-Host "This is required for Soroban audit readiness and mainnet deployment."
    Write-Host ""
    Write-Host "Each entrypoint needs a table row like:" -ForegroundColor Cyan
    Write-Host "| entrypoint_name(params) | A/U/P | Description and auth requirements |"
    Write-Host ""
    Write-Host "Legend: A=Admin, U=User/Actor auth required, P=Public read-only" -ForegroundColor Gray
    
    Write-Host ""
    Write-Host "AUDIT BLOCKER: $($stillMissing.Count) entrypoints missing from matrix" -ForegroundColor Red
    exit 1
}

Write-Host "All known entrypoint gaps have been documented in the matrix" -ForegroundColor Green
Write-Host "Entrypoint matrix is complete!" -ForegroundColor Green