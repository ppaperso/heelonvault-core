<#PSScriptInfo
.VERSION 1.0
.GUID 00000000-0000-0000-0000-000000000000
.AUTHOR Patrick Paysan
.DESCRIPTION HeelonVault - Preparation du dossier de staging pour la generation MSI manuelle
#>

<#
.SYNOPSIS
    Prepare le dossier de staging avec les fichiers necessaires pour la generation MSI.

.DESCRIPTION
    Ce script remplace collect-staging.sh pour Windows. Il prepare le dossier wix/staging/
    avec :
    - Le binaire heelonvault.exe
    - Les DLL GTK4 dependantes (filtrees via ntldd -R)
    - Les DLL critiques GTK4 (liste blanche)
    - Les loaders GDK-Pixbuf
    - Les schemas GLib
    - Le theme Adwaita (filtre)
    - Les icones Adwaita (tailles standard)
    - Les migrations SQL
    - L'icone applicative

.PARAMETER BinaryPath
    Chemin vers le binaire heelonvault.exe compile.

.PARAMETER Msys2Bin
    Chemin vers le dossier bin de MSYS2 (default: C:\msys64\mingw64\bin).

.PARAMETER StagingDir
    Dossier de sortie pour le staging (default: wix\staging).

.PARAMETER OutputDir
    Dossier de sortie pour les fichiers WiX (default: wix\output).

.EXAMPLE
    .\prepare-staging.ps1 -BinaryPath "target\x86_64-pc-windows-msvc\release\heelonvault.exe"

.NOTES
    Necessite :
    - ntldd.exe (MSYS2)
    - glib-compile-schemas.exe (optionnel, pour les schemas)
    - magick (ImageMagick, optionnel, pour la generation de l'icone)

    Test manuel recommande avant utilisation :
    C:\msys64\mingw64\bin\ntldd.exe -R target\distrib\heelonvault-app-x86_64-pc-windows-msvc\heelonvault.exe
#>

param(
    [Parameter(Mandatory=$true)]
    [string]$BinaryPath,
    
    [string]$Msys2Bin = "C:\msys64\mingw64\bin",
    [string]$StagingDir = "wix\staging",
    [string]$OutputDir = "wix\output"
)

# ============================================================
# Initialisation
# ============================================================

Write-Host "[staging] === DEBUT : Preparation du dossier de staging ===" -ForegroundColor Cyan

# Verification des parametres
if (-not (Test-Path $BinaryPath)) {
    Write-Error "[staging] ERREUR : Binaire non trouve : $BinaryPath"
    exit 1
}

if (-not (Test-Path $Msys2Bin)) {
    Write-Error "[staging] ERREUR : MSYS2 bin non trouve : $Msys2Bin"
    exit 1
}

Write-Host "[staging] Binaire     : $BinaryPath" -ForegroundColor DarkGray
Write-Host "[staging] MSYS2       : $Msys2Bin" -ForegroundColor DarkGray
Write-Host "[staging] Staging     : $StagingDir" -ForegroundColor DarkGray
Write-Host "[staging] Output      : $OutputDir" -ForegroundColor DarkGray

# ============================================================
# 1. Nettoyage et creation de la structure
# ============================================================

Write-Host "[staging] Nettoyage de l'ancien dossier de staging..." -ForegroundColor Yellow
Remove-Item -Path $StagingDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $StagingDir | Out-Null

Write-Host "[staging] Creation de la structure des dossiers..." -ForegroundColor Yellow
$dirs = @(
    "$StagingDir\lib\gdk-pixbuf-2.0\2.10.0\loaders",
    "$StagingDir\share\glib-2.0\schemas",
    "$StagingDir\share\themes\Adwaita",
    "$StagingDir\share\icons\Adwaita",
    "$StagingDir\migrations"
)
foreach ($dir in $dirs) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

# ============================================================
# 2. Copie du binaire principal
# ============================================================

Write-Host "[staging] Copie du binaire principal..." -ForegroundColor Yellow
Copy-Item -Path $BinaryPath -Destination "$StagingDir\heelonvault.exe" -Force
Write-Host "[staging] Binaire copie : $StagingDir\heelonvault.exe" -ForegroundColor Green

# ============================================================
# 3. Identification et copie des DLL dependantes (via ntldd -R)
# ============================================================

Write-Host "[staging] Identification des DLL dependantes..." -ForegroundColor Yellow

$ntlddPath = "$Msys2Bin\ntldd.exe"
if (-not (Test-Path $ntlddPath)) {
    Write-Error "[staging] ERREUR : ntldd.exe non trouve a $ntlddPath"
    exit 1
}

$dllCount = 0

# Execution de ntldd -R et parsing de la sortie
try {
    $ntlddOutput = & $ntlddPath -R $BinaryPath 2>&1
    
    # Filtrer les lignes contenant mingw64 ou msys64
    $dllLines = $ntlddOutput | Select-String -Pattern "mingw64|msys64"
    
    if ($dllLines.Count -eq 0) {
        Write-Warning "[staging] ATTENTION : ntldd n'a trouve aucune dependance mingw64/msys64. Verifiez la sortie."
    }
    
    foreach ($line in $dllLines) {
        $lineText = $line.ToString()
        
        # Nettoyer le chemin : enlever tout avant => et tout apres (espace+parentheses)
        $dllPath = $lineText -replace "^.*=>\s*", "" -replace "\s*\(.*", ""
        $dllPath = $dllPath.Trim()
        
        if ([string]::IsNullOrWhiteSpace($dllPath)) {
            continue
        }
        
        # Normaliser le chemin (remplacer / par \ si necessaire)
        $dllPath = $dllPath -replace "/", "\"
        
        # Verifier que le chemin est absolu et pointe vers MSYS2
        if ($dllPath -like "C:\msys64\*" -or $dllPath -like "C:\mingw64\*") {
            if (Test-Path $dllPath) {
                $dest = Join-Path -Path $StagingDir -ChildPath (Split-Path -Path $dllPath -Leaf)
                Copy-Item -Path $dllPath -Destination $dest -Force -ErrorAction SilentlyContinue
                $dllCount++
                Write-Verbose "[staging] DLL copiee : $dllPath"
            } else {
                Write-Warning "[staging] DLL introuvee (mais referencee) : $dllPath"
            }
        }
    }
    
    Write-Host "[staging] $dllCount DLL dependantes identifiees et copiees via ntldd" -ForegroundColor Green
    
} catch {
    Write-Warning "[staging] ATTENTION : Echec du parsing ntldd -R. Utilisation de la liste blanche uniquement."
    Write-Warning "[staging] Erreur : $_"
}

# ============================================================
# 4. Copie des DLL critiques GTK4 (securite - liste blanche)
# ============================================================

Write-Host "[staging] Copie des DLL critiques GTK4 (liste blanche)..." -ForegroundColor Yellow

$criticalDLLs = @(
    "libgtk-4-1.dll",
    "libgraphene-1.0-0.dll",
    "libepoxy-0.dll",
    "libglib-2.0-0.dll",
    "libgobject-2.0-0.dll",
    "libgmodule-2.0-0.dll",
    "libharfbuzz-0.dll",
    "libpango-1.0-0.dll",
    "libpangocairo-1.0-0.dll",
    "libcairo-2.dll",
    "libcairo-gobject-2.dll",
    "libgdk_pixbuf-2.0-0.dll",
    "libgio-2.0-0.dll",
    "libatk-1.0-0.dll",
    "libatkmm-1.6-1.dll",
    "libgdk-4-1.dll"
)

foreach ($dll in $criticalDLLs) {
    $source = Join-Path -Path $Msys2Bin -ChildPath $dll
    if (Test-Path $source) {
        $dest = Join-Path -Path $StagingDir -ChildPath $dll
        if (-not (Test-Path $dest)) {
            Copy-Item -Path $source -Destination $dest -Force
            $dllCount++
            Write-Host "[staging] DLL critique copiee : $dll" -ForegroundColor DarkGray
        }
    } else {
        Write-Warning "[staging] DLL critique introuvee : $dll (source: $source)"
    }
}

Write-Host "[staging] Total DLL : $dllCount" -ForegroundColor Green

# ============================================================
# 5. GDK-Pixbuf loaders (SANS loaders.cache)
# ============================================================

Write-Host "[staging] Copie des loaders GDK-Pixbuf..." -ForegroundColor Yellow
$loadersSrc = "$Msys2Bin\..\lib\gdk-pixbuf-2.0\2.10.0\loaders"
if (Test-Path $loadersSrc) {
    $loadersDest = "$StagingDir\lib\gdk-pixbuf-2.0\2.10.0\loaders"
    New-Item -ItemType Directory -Force -Path $loadersDest | Out-Null
    Copy-Item -Path "$loadersSrc\*.dll" -Destination $loadersDest -Force
    Write-Host "[staging] GDK-Pixbuf loaders copies" -ForegroundColor Green
} else {
    Write-Warning "[staging] Loaders GDK-Pixbuf non trouves a $loadersSrc"
}

# ============================================================
# 6. GLib schemas
# ============================================================

Write-Host "[staging] Copie et compilation des schemas GLib..." -ForegroundColor Yellow
$schemasSrc = "$Msys2Bin\..\share\glib-2.0\schemas"
if (Test-Path $schemasSrc) {
    $schemasDest = "$StagingDir\share\glib-2.0\schemas"
    New-Item -ItemType Directory -Force -Path $schemasDest | Out-Null
    Copy-Item -Path "$schemasSrc\*.xml" -Destination $schemasDest -Force
    
    # Compilation des schemas si glib-compile-schemas est disponible
    if (Get-Command glib-compile-schemas -ErrorAction SilentlyContinue) {
        & glib-compile-schemas $schemasDest
        Write-Host "[staging] Schemas GLib compiles" -ForegroundColor Green
    } else {
        Write-Warning "[staging] glib-compile-schemas non trouve dans PATH. Schemas non compiles."
    }
} else {
    Write-Warning "[staging] Schemas GLib non trouves a $schemasSrc"
}

# ============================================================
# 7. Theme Adwaita (filtre : uniquement CSS, PNG, SVG, index.theme)
# ============================================================

Write-Host "[staging] Copie du theme Adwaita (filtre)..." -ForegroundColor Yellow
$adwaitaSrc = "$Msys2Bin\..\share\themes\Adwaita"
if (Test-Path $adwaitaSrc) {
    $adwaitaFiles = Get-ChildItem -Path $adwaitaSrc -Recurse -File | 
        Where-Object { $_.Extension -in @(".css", ".png", ".svg", ".theme") }
    
    foreach ($file in $adwaitaFiles) {
        $relative = $file.FullName.Substring($adwaitaSrc.Length)
        $dest = Join-Path -Path "$StagingDir\share\themes" -ChildPath $relative
        New-Item -ItemType Directory -Force -Path (Split-Path $dest) | Out-Null
        Copy-Item -Path $file.FullName -Destination $dest -Force
    }
    
    Write-Host "[staging] Theme Adwaita copie (fichiers filtres : $($adwaitaFiles.Count) fichiers)" -ForegroundColor Green
} else {
    Write-Warning "[staging] Theme Adwaita non trouve a $adwaitaSrc"
}

# ============================================================
# 8. Icones Adwaita (tailles standard uniquement)
# ============================================================

Write-Host "[staging] Copie des icones Adwaita (tailles standard)..." -ForegroundColor Yellow
$adwaitaIconsSrc = "$Msys2Bin\..\share\icons\Adwaita"
if (Test-Path $adwaitaIconsSrc) {
    $iconSizes = @("16x16", "22x22", "24x24", "32x32", "48x48", "96x96", "256x256", "scalable")
    $iconCount = 0
    
    foreach ($size in $iconSizes) {
        $sizePath = Join-Path -Path $adwaitaIconsSrc -ChildPath $size
        if (Test-Path $sizePath) {
            $dest = Join-Path -Path "$StagingDir\share\icons\Adwaita" -ChildPath $size
            New-Item -ItemType Directory -Force -Path $dest | Out-Null
            Copy-Item -Path "$sizePath\*" -Destination $dest -Force -Recurse -ErrorAction SilentlyContinue
            $iconCount += (Get-ChildItem -Path $sizePath -File).Count
        }
    }
    
    # Copier index.theme
    $indexTheme = "$adwaitaIconsSrc\index.theme"
    if (Test-Path $indexTheme) {
        Copy-Item -Path $indexTheme -Destination "$StagingDir\share\icons\Adwaita" -Force
    }
    
    Write-Host "[staging] Icones Adwaita copiees ($iconCount fichiers)" -ForegroundColor Green
} else {
    Write-Warning "[staging] Icones Adwaita non trouvees a $adwaitaIconsSrc"
}

# ============================================================
# 9. Migrations SQL
# ============================================================

Write-Host "[staging] Copie des migrations SQL..." -ForegroundColor Yellow
$migrationsSrc = "..\..\migrations"
if (Test-Path $migrationsSrc) {
    $migrationsDest = "$StagingDir\migrations"
    New-Item -ItemType Directory -Force -Path $migrationsDest | Out-Null
    Copy-Item -Path "$migrationsSrc\*.sql" -Destination $migrationsDest -Force
    $sqlCount = (Get-ChildItem -Path $migrationsDest -File).Count
    Write-Host "[staging] $sqlCount migrations SQL copiees" -ForegroundColor Green
} else {
    Write-Warning "[staging] Dossier migrations non trouve : $migrationsSrc"
}

# ============================================================
# 10. Icone applicative
# ============================================================

Write-Host "[staging] Generation de l'icone applicative..." -ForegroundColor Yellow

$iconGenerated = $false

# Methode 1 : Utiliser ImageMagick si disponible
if (Get-Command magick -ErrorAction SilentlyContinue) {
    $iconSrc = "..\..\assets\icons\hicolor\256x256\apps\heelonvault.png"
    if (Test-Path $iconSrc) {
        & magick $iconSrc -define icon:auto-resize=256,128,64,48,32,16 "$StagingDir\heelonvault.ico"
        $iconGenerated = $true
        Write-Host "[staging] Icone generee via ImageMagick" -ForegroundColor Green
    }
}

# Methode 2 : Copier l'icone existante (Heelonys.ico)
if (-not $iconGenerated) {
    # Chercher dans plusieurs emplacements
    $possibleIcons = @(
        "..\..\wix\Heelonys.ico",
        "..\..\crates\heelonvault-app\wix\Heelonys.ico",
        "$Msys2Bin\..\wix\Heelonys.ico",
        "$Msys2Bin\..\Heelonys.ico"
    )
    
    foreach ($iconPath in $possibleIcons) {
        if (Test-Path $iconPath) {
            Copy-Item -Path $iconPath -Destination "$StagingDir\heelonvault.ico" -Force
            $iconGenerated = $true
            Write-Host "[staging] Icone copinee depuis : $iconPath" -ForegroundColor Green
            break
        }
    }
}

if (-not $iconGenerated) {
    Write-Warning "[staging] AUCUNE ICONE TROUVEE. L'installateur n'aura pas d'icone personnalisee."
}

# ============================================================
# 11. Nettoyage des fichiers indesirables
# ============================================================

Write-Host "[staging] Nettoyage des fichiers indesirables..." -ForegroundColor Yellow

$exclusions = @("*.pdb", "*.a", "*.lib", "*.def", "*.cache", "*symbolic.svg")
$cleanedCount = 0

foreach ($pattern in $exclusions) {
    $files = Get-ChildItem -Path $StagingDir -Recurse -File -Filter $pattern -ErrorAction SilentlyContinue
    foreach ($file in $files) {
        Remove-Item -Path $file.FullName -Force -ErrorAction SilentlyContinue
        $cleanedCount++
    }
}

Write-Host "[staging] $cleanedCount fichiers indesirables supprimes" -ForegroundColor Green

# ============================================================
# 12. Creation du dossier de sortie
# ============================================================

Write-Host "[staging] Creation du dossier de sortie..." -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

# ============================================================
# Statistiques finales
# ============================================================

$totalFiles = (Get-ChildItem -Path $StagingDir -Recurse -File).Count
$totalSize = (Get-ChildItem -Path $StagingDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
$totalSizeMB = [math]::Round($totalSize / 1MB, 2)

Write-Host "`n[staging] === FIN : Preparation du staging terminee ===" -ForegroundColor Cyan
Write-Host "[staging] Fichiers totaux   : $totalFiles" -ForegroundColor Green
Write-Host "[staging] Taille totale    : $totalSizeMB Mo" -ForegroundColor Green
Write-Host "[staging] DLL              : $dllCount" -ForegroundColor Green
Write-Host "[staging] Dossier          : $StagingDir" -ForegroundColor Green
Write-Host "[staging] Sortie           : $OutputDir" -ForegroundColor Green

if (-not $iconGenerated) {
    Write-Warning "[staging] ATTENTION : Aucune icone n'a ete trouvée. L'installateur utilisera une icone par defaut."
}

Write-Host "[staging] Prepret pour la generation MSI avec candle.exe et light.exe" -ForegroundColor Cyan
