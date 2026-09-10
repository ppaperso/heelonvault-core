#requires -version 5.1
<#
.SYNOPSIS
    Build MSI HeelonVault - recette propre en 4 etapes.
.DESCRIPTION
    1. cargo build --release
    2. Assemblage du staging (exe + runtime GTK bin/lib/share, AUCUNE DLL systeme)
    3. heat.exe  -> staging.wxs   (no -b, no path rewriting)
    4. candle.exe + light.exe     -> HeelonVault-<version>.msi
.EXAMPLE
    .\build-msi.ps1 -GtkRoot "C:\msys64\mingw64"
#>

# --- Paramètres ---
param(
    [string]$BaseDir = "",  # Chemin auto-détecté ou à préciser
    [string]$GtkRoot = "",  # Chemin auto-détecté ou à préciser
    [string]$WixBin  = "",  # Chemin auto-détecté ou à préciser
    [string]$Version = ""   # vide = lu depuis Cargo.toml
)

# --- Fonctions utilitaires ---
function Die($msg) { Write-Host "[FAIL] $msg" -ForegroundColor Red; exit 1 }
function Ok($msg)  { Write-Host "[PASS] $msg" -ForegroundColor Green }

# --- Détection automatique des chemins ---
if (-not $BaseDir -or $BaseDir -eq "") {
    if ($PSScriptRoot) {
        # Le script est dans scripts/, on remonte d'un niveau pour atteindre heelonvault-core
        $BaseDir = (Split-Path $PSScriptRoot -Parent)
    } else {
        $BaseDir = $PWD
    }
    
    # Validation de la présence du Cargo.toml
    if (-not (Test-Path (Join-Path $BaseDir "Cargo.toml"))) {
        Die "Workspace root not found. Specify -BaseDir."
    }
}

if (-not $GtkRoot -or $GtkRoot -eq "") {
    if (Test-Path "C:\msys64\mingw64\bin") {
        $GtkRoot = "C:\msys64\mingw64"
    } else {
        $gtkPath = (Get-Command gtk4-node.exe -ErrorAction SilentlyContinue).Source
        if ($gtkPath) {
            $GtkRoot = Split-Path (Split-Path $gtkPath -Parent) -Parent
        } else {
            Die "GTK4 not found in PATH. Specify -GtkRoot."
        }
    }
}

if (-not $WixBin -or $WixBin -eq "") {
    if (Test-Path "C:\Program Files (x86)\WiX Toolset v3.14\bin\candle.exe") {
        $WixBin = "C:\Program Files (x86)\WiX Toolset v3.14\bin"
    } elseif (Test-Path "C:\Program Files\WiX Toolset v3.14\bin\candle.exe") {
        $WixBin = "C:\Program Files\WiX Toolset v3.14\bin"
    } else {
        Die "WiX Toolset not found. Specify -WixBin."
    }
}

Ok "Chemins détectés - BaseDir: $BaseDir | GtkRoot: $GtkRoot | WixBin: $WixBin"

# Mettre à jour PATH avec le GtkRoot détecté
$env:PATH = "$GtkRoot\bin;$GtkRoot\..\usr\bin;" + $env:PATH
$ErrorActionPreference = "Stop"
$AppDir  = Join-Path $BaseDir "crates\heelonvault-app"
$WixDir  = Join-Path $AppDir "wix"
$Staging = Join-Path $WixDir "staging"
$OutDir  = Join-Path $WixDir "output"

# ---------------------------------------------------------------
# 0. Version
# ---------------------------------------------------------------
if (-not $Version) {
    $cargoToml = Join-Path $AppDir "Cargo.toml"
    $Version = (Select-String -Path $cargoToml -Pattern '^version\s*=\s*"([^"]+)"' |
                ForEach-Object { $_.Matches[0].Groups[1].Value })
    if (-not $Version) { Die "Version not found in $cargoToml" }
}

# Nettoyage pour WiX (qui n'accepte que des chiffres x.y.z.w)
# On extrait la partie num�rique de base (ex: 1.2.0)
$CleanVersion = $Version -replace '-.*$', ''
$vParts = $CleanVersion.Split('.')

# Si la version contient un -rc.X, on r�cup�re le chiffre pour en faire le 4e segment (ex: 1.2.0.1)
if ($Version -match '-rc\.(\d+)') {
    $rcNum = $Matches[1]
    while ($vParts.Count -lt 3) { $vParts += "0" }
    $vParts += $rcNum
} else {
    # Sinon on compl�te avec des 0 pour atteindre 4 segments (ex: 1.2.0.0)
    while ($vParts.Count -lt 4) { $vParts += "0" }
}
$WixVersion = $vParts[0..3] -join '.'

Ok "Version: $Version (WiX: $WixVersion)"

# ---------------------------------------------------------------
# 1. Build Rust
# ---------------------------------------------------------------
Write-Host "`n=== [1/4] cargo build --release ===" -ForegroundColor Cyan
Push-Location $BaseDir
cargo build --release -p heelonvault-app
if ($LASTEXITCODE -ne 0) { Pop-Location; Die "cargo build failed" }
Pop-Location
$exe = Join-Path $BaseDir "target\release\heelonvault.exe"
if (-not (Test-Path $exe)) { Die "Binary not found: $exe" }
Ok "Binary: $exe"

# ---------------------------------------------------------------
# 2. Assemblage du staging (propre, recreable, sans DLL systeme)
# ---------------------------------------------------------------
Write-Host "`n=== [2/4] Assemblage staging ===" -ForegroundColor Cyan
if (Test-Path $Staging) { Remove-Item $Staging -Recurse -Force }
New-Item -ItemType Directory -Force -Path "$Staging\bin" | Out-Null

# 2a. Le binaire
Copy-Item $exe "$Staging\bin\heelonvault.exe"

function Get-RequiredDlls {
    param(
        [Parameter(Mandatory)] [string]$ExePath,
        [Parameter(Mandatory)] [string]$SearchDir
    )
    $resolved = New-Object System.Collections.Generic.HashSet[string]
    $queue    = New-Object System.Collections.Generic.Queue[string]
    $queue.Enqueue($ExePath)

    while ($queue.Count -gt 0) {
        $current = $queue.Dequeue()
        $imports = & objdump -p $current 2>$null |
                   Select-String '^\s*DLL Name:\s*(\S+)' |
                   ForEach-Object { $_.Matches[0].Groups[1].Value }

        foreach ($dll in $imports) {
            $candidate = Join-Path $SearchDir $dll
            if ((Test-Path $candidate) -and -not $resolved.Contains($dll)) {
                $resolved.Add($dll) | Out-Null
                $queue.Enqueue($candidate)
            }
        }
    }
    return $resolved
}

# 2b. Uniquement les DLL r�ellement requises (fermeture transitive des imports
# PE), et non tout mingw64\bin (qui contient aussi l'outillage de dev MSYS2
# sans rapport avec l'app). Reduit fortement le nombre de fichiers embarques.
$requiredDlls = Get-RequiredDlls -ExePath $exe -SearchDir (Join-Path $GtkRoot "bin")
foreach ($dll in $requiredDlls) {
    Copy-Item (Join-Path $GtkRoot "bin\$dll") "$Staging\bin\"
}
$dllCount = (Get-ChildItem "$Staging\bin\*.dll").Count
Ok "$dllCount DLLs copied (dependency closure) from $($GtkRoot)\bin"

# 2c. gdk-pixbuf loaders (+ cache)
New-Item -ItemType Directory -Force -Path "$Staging\lib" | Out-Null
Copy-Item (Join-Path $GtkRoot "lib\gdk-pixbuf-2.0") "$Staging\lib\" -Recurse
Ok "gdk-pixbuf-2.0 copied (lib\)"

# 2d. share : schemas GLib, ressources GTK4, icones, polices
foreach ($sub in @("glib-2.0", "gtk-4.0", "icons", "fonts")) {
    $src = Join-Path $GtkRoot "share\$sub"
    if (Test-Path $src) {
        New-Item -ItemType Directory -Force -Path "$Staging\share" | Out-Null
        Copy-Item $src "$Staging\share\" -Recurse
        Ok "share\$sub copied"
    } else {
        Write-Host "[WARN] $src not found (ignored)" -ForegroundColor Yellow
    }
}

# 2e. (Optionnel) assets runtime de l'app si NON embarques dans le binaire.
#     Decommenter si heelonvault.exe attend des fichiers sur disque :
# Copy-Item (Join-Path $BaseDir "assets")    "$Staging\share\heelonvault\" -Recurse -ErrorAction SilentlyContinue
# Copy-Item (Join-Path $BaseDir "resources") "$Staging\share\heelonvault\" -Recurse -ErrorAction SilentlyContinue

# ---------------------------------------------------------------
# 2f. Migrations SQLx (OBLIGATOIRE - cause du crash si manquant)
# ---------------------------------------------------------------
$MigrationsSrc = Join-Path $AppDir "migrations"
if (-not (Test-Path $MigrationsSrc)) {
    Die "migrations/ directory not found at: $MigrationsSrc"
}
$TargetMigrationsDir = Join-Path "$Staging\bin" "migrations"
New-Item -ItemType Directory -Force -Path $TargetMigrationsDir | Out-Null
Copy-Item "$MigrationsSrc\*" $TargetMigrationsDir -Recurse -Force
Ok "migrations/ copied to bin\migrations (CRITICAL for SQLx)"

# ---------------------------------------------------------------
# 2f. Ressources applicatives (assets + locales)
# ---------------------------------------------------------------
# Note: assets/ est maintenant dans crates/heelonvault-app/
$assetsSrc = Join-Path $AppDir "assets"
if (Test-Path $assetsSrc) {
    $targetAssetsDir = Join-Path "$Staging\share\heelonvault" "assets"
    New-Item -ItemType Directory -Force -Path $targetAssetsDir | Out-Null
    Copy-Item "$assetsSrc\*" $targetAssetsDir -Recurse -Force
    Ok "assets/ copied to share\heelonvault\assets"
}

# Fichiers de traduction FTL (fallback pour i18n - embarqués dans le binaire normalement)
# Note: locales/ est maintenant dans crates/heelonvault-core/
$localesSrc = Join-Path $BaseDir "crates\heelonvault-core\locales"
if (Test-Path $localesSrc) {
    $targetLocalesDir = Join-Path "$Staging\share\heelonvault" "locales"
    New-Item -ItemType Directory -Force -Path $targetLocalesDir | Out-Null
    Copy-Item "$localesSrc\*" $targetLocalesDir -Recurse -Force
    Ok "locales/ copied to share\heelonvault\locales (i18n fallback)"
}

# ---------------------------------------------------------------
# 3. heat : staging.wxs + Correction du pr�fixe SourceDir
# ---------------------------------------------------------------
Write-Host "`n=== [3/4] heat ===" -ForegroundColor Cyan
Push-Location $WixDir

# G�n�ration du staging.wxs via heat
# -var var.StagingDir : heat ecrit Source="$(var.StagingDir)\..." au lieu du
# jeton litt�ral "SourceDir\...". Ce jeton par defaut est resolu par light au
# binding, et WiX v3 a un bug connu (wixtoolset/issues#4439) ou cette
# resolution casse des qu'un argument de binder est passe a light (-ext, -b,
# -spdb, peu importe lequel). var.StagingDir, lui, est une variable de
# PREPROCESSEUR candle : elle est substituee a la COMPILATION, avant meme que
# light entre en jeu. Plus besoin de -b ni de reecriture manuelle du xml.
& (Join-Path $WixBin "heat.exe") dir "staging" -out "staging.wxs" `
    -dr INSTALLFOLDER -cg StagingComponents -gg -sfrag -srd -sreg -var var.StagingDir
if ($LASTEXITCODE -ne 0) { Pop-Location; Die "heat failed" }

Ok "staging.wxs generated (Source linked to `$(var.StagingDir)))"
Pop-Location

# ---------------------------------------------------------------
# 4. candle + light
# ---------------------------------------------------------------
Write-Host "`n=== [4/4] candle + light ===" -ForegroundColor Cyan
Push-Location $WixDir

if (Test-Path $OutDir) { 
    Remove-Item $OutDir -Recurse -Force 
    Ok "output directory cleaned"
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$candleOutput = & (Join-Path $WixBin "candle.exe") "main.wxs" "staging.wxs" `
    -arch x64 -out "$OutDir\" -ext WixUtilExtension "-dProductVersion=$WixVersion" "-dStagingDir=$Staging" 2>&1

if ($LASTEXITCODE -ne 0) {
    Write-Host $candleOutput -ForegroundColor Red
    Pop-Location
    Die "candle failed"
}
Ok "compilation candle OK"

$msi = Join-Path $OutDir "HeelonVault-$Version.msi"
# Plus besoin de -b : StagingDir a deja ete resolu en chemin absolu par candle
# (variable de preprocesseur, cf. commentaire a l'etape heat plus haut).
$lightOutput = & (Join-Path $WixBin "light.exe") "$OutDir\main.wixobj" "$OutDir\staging.wixobj" `
    -out "$msi" -ext WixUtilExtension -spdb -dcl:high 2>&1

if ($LASTEXITCODE -ne 0) {
    Write-Host $lightOutput -ForegroundColor Red
    Pop-Location
    Die "light failed"
}
Pop-Location

$sizeMB = [math]::Round((Get-Item $msi).Length / 1MB, 2)
Ok "MSI generated: $msi ($sizeMB MB)"
