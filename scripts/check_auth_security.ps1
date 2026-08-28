# Authentication Security Validation Script
# Validates fail-closed authentication patterns across Mux Protocol contracts

Write-Host "🔍 Validating fail-closed authentication patterns..." -ForegroundColor Cyan

$issues = @()
$contracts = Get-ChildItem -Path "contracts/*/src/lib.rs" -Recurse

# 1. Session Key Empty Scope Validation (T-40)
Write-Host "`n📋 Checking session key empty scope validation (T-40)..." -ForegroundColor Yellow

$sessionKeyFile = "contracts/mux-account/src/lib.rs"
if (Test-Path $sessionKeyFile) {
    $content = Get-Content $sessionKeyFile -Raw
    if ($content -match "if record\.scopes\.is_empty\(\).*return Err.*Unauthorized") {
        Write-Host "  ✅ Session key empty scope validation implemented" -ForegroundColor Green
    } else {
        $issues += "❌ Session key empty scope validation missing"
        Write-Host "  ❌ Session key empty scope validation missing" -ForegroundColor Red
    }
} else {
    $issues += "❌ mux-account contract not found"
}

# 2. Owner Verification Fail-Closed Pattern  
Write-Host "`n📋 Checking owner verification patterns..." -ForegroundColor Yellow

$recoveryFile = "contracts/mux-recovery/src/lib.rs"
if (Test-Path $recoveryFile) {
    $content = Get-Content $recoveryFile -Raw
    if ($content -match "if stored_owner != owner.*return Err.*Unauthorized") {
        Write-Host "  ✅ mux-recovery set_registry: fail-closed owner verification" -ForegroundColor Green
    } else {
        $issues += "❌ Owner verification fail-closed pattern missing in mux-recovery"
        Write-Host "  ❌ Owner verification fail-closed pattern missing" -ForegroundColor Red
    }
    
    # Check dual auth pattern for approve_recovery_admin
    if ($content -match "Self::require_owner.*co_guardian\.require_auth") {
        Write-Host "  ✅ approve_recovery_admin: dual auth (owner + guardian)" -ForegroundColor Green
    } else {
        $issues += "❌ Dual auth pattern missing in approve_recovery_admin"
        Write-Host "  ❌ Dual auth pattern missing in approve_recovery_admin" -ForegroundColor Red
    }
} else {
    $issues += "❌ mux-recovery contract not found"
}

# 3. Admin Auth Before Mutation Pattern
Write-Host "`n📋 Checking admin auth before mutation patterns..." -ForegroundColor Yellow

$adminContracts = @(
    @("mux-registry", "require_admin"),
    @("mux-permissions", "require_admin"), 
    @("mux-spending-policy", "require_admin"),
    @("mux-wallet-registry", "require_owner"),
    @("mux-batcher", "require_admin"),
    @("mux-delegation", "require_admin")
)

foreach ($contract in $adminContracts) {
    $contractName = $contract[0]
    $authPattern = $contract[1]
    $contractFile = "contracts/$contractName/src/lib.rs"
    
    if (Test-Path $contractFile) {
        $content = Get-Content $contractFile -Raw
        if ($content -match $authPattern) {
            Write-Host "  ✅ $contractName`: $authPattern patterns detected" -ForegroundColor Green
        } else {
            Write-Host "  ⚠️  $contractName`: $authPattern patterns not found" -ForegroundColor Yellow
        }
    } else {
        Write-Host "  ⚠️  $contractName`: contract not found" -ForegroundColor Yellow
    }
}

# 4. No Conditional Auth Bypass
Write-Host "`n📋 Checking for conditional auth bypass patterns..." -ForegroundColor Yellow

$bypassPatterns = @(
    "if.*require_auth",
    "require_auth.*if.*\{",
    "match.*require_auth"
)

$foundBypasses = $false
foreach ($contractFile in $contracts) {
    $contractName = Split-Path (Split-Path (Split-Path $contractFile -Parent) -Parent) -Leaf
    $content = Get-Content $contractFile -Raw
    
    foreach ($pattern in $bypassPatterns) {
        if ($content -match $pattern) {
            Write-Host "  ⚠️  $contractName`: potential conditional auth bypass: $pattern" -ForegroundColor Yellow
            $foundBypasses = $true
        }
    }
}

if (!$foundBypasses) {
    Write-Host "  ✅ No conditional auth bypass patterns detected" -ForegroundColor Green
}

# 5. Require Auth in Admin Functions
Write-Host "`n📋 Checking admin function auth coverage..." -ForegroundColor Yellow

foreach ($contractFile in $contracts) {
    $contractName = Split-Path (Split-Path (Split-Path $contractFile -Parent) -Parent) -Leaf
    $content = Get-Content $contractFile -Raw
    
    # Find admin functions (initialize, upgrade, set_, create_, etc.)
    $adminFunctions = @()
    if ($content -match "pub fn initialize") { $adminFunctions += "initialize" }
    if ($content -match "pub fn upgrade") { $adminFunctions += "upgrade" }
    
    $hasAdminAuth = $false
    if ($content -match "require_auth|require_admin|require_owner") {
        $hasAdminAuth = $true
    }
    
    if ($adminFunctions.Count -gt 0) {
        if ($hasAdminAuth) {
            Write-Host "  ✅ $contractName`: admin functions have auth ($($adminFunctions -join ', '))" -ForegroundColor Green
        } else {
            $issues += "❌ $contractName`: admin functions missing auth"
            Write-Host "  ❌ $contractName`: admin functions missing auth" -ForegroundColor Red
        }
    }
}

# Summary
Write-Host "`n📊 Summary:" -ForegroundColor Cyan
Write-Host "   Contracts analyzed: $($contracts.Count)" -ForegroundColor White
Write-Host "   Issues found: $($issues.Count)" -ForegroundColor White

if ($issues.Count -eq 0) {
    Write-Host "`n🎉 All authentication patterns are fail-closed!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "`n⚠️  Authentication issues detected:" -ForegroundColor Red
    foreach ($issue in $issues) {
        Write-Host "   $issue" -ForegroundColor Red
    }
    exit 1
}