<#
.SYNOPSIS
Builds the Husker online server and deploys it to Azure Functions.

.DESCRIPTION
The script runs these steps. Each one is safe to repeat.

1. Builds the Linux binary and reads the public key of your Husker profile.
2. Packages the binary with host.json and serve/function.json.
3. Checks the stored access policy for the server's access to the sync file. With -CreatePolicy it
   creates a missing policy.
4. Deploys main.bicep: the function app, its plan, and its own storage account.
5. Uploads the package and deploys code.bicep, which hands the package to the function app.
6. Waits until /api/health answers.

The first deployment needs -CreatePolicy and is read-only. Run the script again with -AllowWrites
after you compared what the server reads with a computer.

Never create a policy again under a name you deleted to cut the server off. Azure then accepts every
old SAS of that name again. Pass a new name with -SyncPolicy instead.

The script prints no secret. It runs on Windows and needs the Azure CLI, cargo, zig, cargo-zigbuild,
and the Rust target x86_64-unknown-linux-musl.

Every run sets the read-only switch, the revoked token IDs, and the time zone from its parameters.
A run without -AllowWrites makes the server read-only again, and a run without -Revoked clears the list.

.EXAMPLE
./deploy/azure/deploy.ps1 -Subscription main -ResourceGroup husker -SyncAccount mysyncaccount -App husker-online-example -CreatePolicy
#>
[CmdletBinding()]
param(
    # The Azure subscription that holds the sync file. It is passed to every az command.
    [Parameter(Mandatory = $true)] [string] $Subscription,
    # The resource group of the sync storage account. The function app goes into the same group.
    [Parameter(Mandatory = $true)] [string] $ResourceGroup,
    # The storage account that holds the sync file.
    [Parameter(Mandatory = $true)] [string] $SyncAccount,
    # The name of the function app. The address becomes https://<App>.azurewebsites.net.
    [Parameter(Mandatory = $true)] [string] $App,
    # The new storage account for the function host. The default derives it from -App.
    [string] $HostAccount,
    [string] $SyncContainer = 'husker',
    [string] $SyncBlob = 'tasks.json',
    [string] $SyncPolicy = 'husker-online',
    # The IANA time zone that decides "today" for MCP calls.
    [string] $TimeZone = 'Europe/Copenhagen',
    # Token IDs that the server refuses, for example -Revoked 0123456789ab,ba9876543210.
    [string[]] $Revoked = @(),
    # The Husker profile that holds online.toml. The default is ~/.husker.
    [string] $HuskerProfile,
    [int] $MaxInstances = 1,
    # Lets tokens with write access change tasks. Leave it out for the first deployment.
    [switch] $AllowWrites,
    # Creates the stored access policy when it is missing. Use it for the first deployment,
    # or together with a new -SyncPolicy name after you deleted the old policy.
    [switch] $CreatePolicy,
    # Reuses the binaries from an earlier run.
    [switch] $SkipBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$target = 'x86_64-unknown-linux-musl'
if (-not $HostAccount) {
    $HostAccount = (($App.ToLowerInvariant() -replace '[^a-z0-9]', '') + 'host')
    if ($HostAccount.Length -gt 24) { $HostAccount = $HostAccount.Substring(0, 24) }
}

function Assert-Success([string] $Step) {
    if ($LASTEXITCODE -ne 0) { throw "$Step failed." }
}

# A misspelled ID would revoke nothing, so each one is checked here and again by the server.
foreach ($id in $Revoked) {
    if ($id -cnotmatch '^[0-9a-f]{12}$') {
        throw "'$id' is not a token ID. A token ID has 12 lowercase hexadecimal characters."
    }
}
$revokedList = $Revoked -join ','

Push-Location $repo
try {
    if (-not $SkipBuild) {
        Write-Host 'Building husker for this computer and for Linux...'
        cargo build --release --locked
        Assert-Success 'The build for this computer'
        cargo zigbuild --release --locked --target $target
        Assert-Success 'The Linux build'
    }

    $husker = Join-Path $repo 'target\release\husker.exe'
    $profileArguments = @()
    $profileDirectory = Join-Path $HOME '.husker'
    if ($HuskerProfile) {
        $profileArguments = @('--profile', $HuskerProfile)
        $profileDirectory = $HuskerProfile
    }
    if (-not (Test-Path (Join-Path $profileDirectory 'online.toml'))) {
        Write-Warning "No online.toml in $profileDirectory. Husker creates a new key now. Access tokens that another key signed stop working after this deployment."
    }
    $publicKey = & $husker @profileArguments token --public-key
    Assert-Success 'Reading the public key'

    $version = (Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
    $stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMdd-HHmmss')
    $package = "husker-$version-$stamp.zip"
    $zip = Join-Path $repo "target\$package"
    cargo run --quiet --release --locked --example package -- (Join-Path $repo "target\$target\release\husker") $zip
    Assert-Success 'Packaging'

    Write-Host "Checking the stored access policy $SyncPolicy..."
    $policy = az storage container policy show --account-name $SyncAccount --container-name $SyncContainer `
        --name $SyncPolicy --auth-mode key --subscription $Subscription --only-show-errors -o json 2>$null
    if ($LASTEXITCODE -ne 0 -or -not $policy -or "$policy".Trim() -eq '{}') {
        if (-not $CreatePolicy) {
            throw ("The stored access policy '$SyncPolicy' was not found or could not be read. " +
                "For a first deployment, run again with -CreatePolicy. " +
                "If you deleted the policy to cut the server off, also pass a new name with -SyncPolicy: " +
                "a policy created again under an old name makes every old SAS of that name valid again.")
        }
        # Read and write. The online server never creates the sync file: it uploads only with If-Match.
        az storage container policy create --account-name $SyncAccount --container-name $SyncContainer `
            --name $SyncPolicy --permissions rw --expiry '2036-01-01T00:00:00Z' `
            --auth-mode key --subscription $Subscription --only-show-errors -o none
        Assert-Success 'Creating the stored access policy'
    }

    Write-Host "Deploying the function app $App..."
    $readOnly = if ($AllowWrites) { 'false' } else { 'true' }
    $url = az deployment group create --subscription $Subscription --resource-group $ResourceGroup `
        --name 'husker-online' --template-file (Join-Path $PSScriptRoot 'main.bicep') `
        --parameters app=$App hostAccount=$HostAccount syncAccount=$SyncAccount syncContainer=$SyncContainer `
        syncBlob=$SyncBlob syncPolicy=$SyncPolicy onlineKeys=$publicKey revoked=$revokedList timeZone=$TimeZone `
        readOnly=$readOnly maxInstances=$MaxInstances `
        --query 'properties.outputs.url.value' -o tsv --only-show-errors
    Assert-Success 'The infrastructure deployment'

    Write-Host "Uploading $package..."
    az storage blob upload --account-name $HostAccount --container-name packages --name $package --file $zip `
        --auth-mode key --subscription $Subscription --overwrite --only-show-errors -o none
    Assert-Success 'The package upload'

    az deployment group create --subscription $Subscription --resource-group $ResourceGroup `
        --name 'husker-online-code' --template-file (Join-Path $PSScriptRoot 'code.bicep') `
        --parameters app=$App hostAccount=$HostAccount packageBlob=$package -o none --only-show-errors
    Assert-Success 'The code deployment'

    Write-Host "Waiting for $url/api/health..."
    $health = $null
    foreach ($attempt in 1..30) {
        try {
            $health = Invoke-RestMethod -Uri "$url/api/health" -TimeoutSec 30
            if ($health.version -eq $version) { break }
        } catch {
            $health = $null
        }
        Start-Sleep -Seconds 5
    }
    if (-not $health -or $health.version -ne $version) {
        throw "The server did not answer with version $version at $url/api/health."
    }
    Write-Host "Husker $($health.version) is online at $url. Writes allowed: $([bool]$AllowWrites)."
    Write-Host "Next: husker token --url $url --name NAME"
} finally {
    Pop-Location
}
