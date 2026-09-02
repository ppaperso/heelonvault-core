<#
.SYNOPSIS
    Script d'installation automatique de l'environnement de build MSI HeelonVault sur Windows 11.
.DESCRIPTION
    Installe tous les outils nécessaires :
    - .NET SDK 8 (pour WiX v7)
    - MSYS2 MINGW64 avec les paquets de compilation
    - Rustup et toolchain GNU
    - WiX v7 (outil global .NET)
    Configure le PATH de MSYS2 pour accéder à Git, dotnet et wix.
    Vérifie que toutes les commandes sont disponibles.
.NOTES
    À exécuter dans PowerShell en tant qu'administrateur.
    Le script est idempotent : il vérifie l'existence des outils avant de les installer.
    Après exécution, redémarrez MSYS2 MINGW64 pour appliquer les changements de PATH.
#>

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

# Chemins
$msys2Root = "C:\msys64"
$msys2Bash = Join-Path $msys2Root "usr\bin\bash.exe"
$msys2Profile = Join-Path $msys2Root "home\$env:USERNAME\.bashrc"
$msys2ProfileGlobal = Join-Path $msys2Root "etc\profile"
$msys2BashProfile = Join-Path $msys2Root "home\$env:USERNAME\.bash_profile"

# Vérifier les droits administrateur
if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] "Administrator")) {
    Write-Host "Ce script nécessite des droits administrateur. Relancement en tant qu'administrateur..." -ForegroundColor Yellow
    Start-Process powershell -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    exit
}

# Fonctions utilitaires
function Test-CommandExists {
    param([string]$Command)
    $null -ne (Get-Command $Command -ErrorAction SilentlyContinue)
}

# Définit l'environnement MSYS2 pour chaque appel
function Invoke-Msys2Command {
    param([string]$Command, [string]$Description = "")
    if ($Description) { Write-Host ">> $Description" -ForegroundColor Cyan }

    $env:MSYSTEM = "MINGW64"
    $env:MSYS2_PATH_TYPE = "minimal"   # Assure un PATH propre avec /mingw64/bin et /usr/bin

    # Utiliser --login -c pour charger /etc/profile
    & $msys2Bash --login -c $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Échec de la commande MSYS2 : $Command"
    }
}

function Add-ContentIfNotExists {
    param([string]$Path, [string]$Line)
    if (-not (Select-String -Path $Path -SimpleMatch $Line -Quiet -ErrorAction SilentlyContinue)) {
        Add-Content -Path $Path -Value $Line
        Write-Host "Ajouté à $Path : $Line" -ForegroundColor Green
    } else {
        Write-Host "Déjà présent dans $Path : $Line" -ForegroundColor Gray
    }
}

function Remove-LineMatching {
    param([string]$Path, [string]$Pattern)
    if (Test-Path $Path) {
        $content = Get-Content $Path
        $newContent = $content | Where-Object { $_ -notmatch $Pattern }
        Set-Content -Path $Path -Value $newContent
    }
}

# 1. Vérifier winget
Write-Host "`n=== Vérification de winget ===" -ForegroundColor Cyan
if (-not (Test-CommandExists "winget")) {
    throw "winget n'est pas disponible. Installez-le depuis le Microsoft Store ou via https://aka.ms/getwinget"
}
Write-Host "winget est disponible."

# 2. Installer .NET SDK 8
Write-Host "`n=== .NET SDK 8 ===" -ForegroundColor Cyan
if (Test-CommandExists "dotnet") {
    $dotnetVersion = & dotnet --version
    if ($dotnetVersion -like "8.*") {
        Write-Host ".NET SDK 8 déjà installé (version $dotnetVersion)." -ForegroundColor Green
    } else {
        Write-Host "Version .NET différente détectée ($dotnetVersion). Installation de .NET SDK 8..." -ForegroundColor Yellow
        winget install --id Microsoft.DotNet.SDK.8 -e --source winget --accept-package-agreements --accept-source-agreements
    }
} else {
    Write-Host "Installation de .NET SDK 8..." -ForegroundColor Yellow
    winget install --id Microsoft.DotNet.SDK.8 -e --source winget --accept-package-agreements --accept-source-agreements
}

# 3. Installer MSYS2
Write-Host "`n=== MSYS2 ===" -ForegroundColor Cyan
if (Test-Path $msys2Root) {
    Write-Host "MSYS2 déjà installé dans $msys2Root." -ForegroundColor Green
} else {
    Write-Host "Installation de MSYS2..." -ForegroundColor Yellow
    winget install --id MSYS2.MSYS2 -e --source winget --accept-package-agreements --accept-source-agreements
    if (-not (Test-Path $msys2Root)) {
        throw "L'installation de MSYS2 a échoué."
    }
}

# 4. Mise à jour initiale de MSYS2 (en une seule commande Syu)
Write-Host "`n=== Mise à jour de MSYS2 ===" -ForegroundColor Cyan
Invoke-Msys2Command "pacman -Syu --noconfirm" "Synchronisation et mise à jour des paquets de base"

# 5. Installation des paquets nécessaires
Write-Host "`n=== Paquets MSYS2 ===" -ForegroundColor Cyan
$msys2Packages = @(
    "git",
    "curl",
    "mingw-w64-x86_64-pkgconf",
    "mingw-w64-x86_64-ntldd",
    "mingw-w64-x86_64-imagemagick",
    "mingw-w64-x86_64-glib2",
    "mingw-w64-x86_64-gdk-pixbuf2",
    "mingw-w64-x86_64-gtk4",
    "mingw-w64-x86_64-graphene",
    "mingw-w64-x86_64-libepoxy",
    "mingw-w64-x86_64-libadwaita",
    "python3"
)
$packageList = $msys2Packages -join " "
Invoke-Msys2Command "pacman -S --needed --noconfirm $packageList" "Installation des paquets de compilation et de dépendances"

# 6. Installation de rustup dans MSYS2
Write-Host "`n=== Rustup dans MSYS2 ===" -ForegroundColor Cyan
$cargoCheck = & $msys2Bash --login -c "command -v cargo && cargo --version" 2>$null
if ($LASTEXITCODE -eq 0 -and $cargoCheck -match "cargo") {
    Write-Host "Rustup/cargo déjà installé dans MSYS2." -ForegroundColor Green
} else {
    Write-Host "Installation de rustup dans MSYS2..." -ForegroundColor Yellow
    Invoke-Msys2Command "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal" "Téléchargement et installation de rustup"
}
Add-ContentIfNotExists -Path $msys2Profile -Line 'export PATH="$HOME/.cargo/bin:$PATH"'
Add-ContentIfNotExists -Path $msys2ProfileGlobal -Line 'export PATH="$HOME/.cargo/bin:$PATH"'

# S'assurer que ~/.bash_profile source ~/.bashrc
if (-not (Test-Path $msys2BashProfile)) {
    Set-Content -Path $msys2BashProfile -Value 'test -f ~/.bashrc && . ~/.bashrc'
    Write-Host "Créé $msys2BashProfile avec le source de ~/.bashrc." -ForegroundColor Green
} elseif (-not (Select-String -Path $msys2BashProfile -SimpleMatch '. ~/.bashrc' -Quiet -ErrorAction SilentlyContinue)) {
    Add-Content -Path $msys2BashProfile -Value 'test -f ~/.bashrc && . ~/.bashrc'
    Write-Host "Ajout du source de ~/.bashrc dans $msys2BashProfile." -ForegroundColor Green
}

# 7. Installation de WiX v7
Write-Host "`n=== WiX v7 ===" -ForegroundColor Cyan
if (Test-CommandExists "wix") {
    $wixVersion = & wix --version 2>$null
    if ($wixVersion -match "^7\.") {
        Write-Host "WiX v7 déjà installé (version $wixVersion)." -ForegroundColor Green
    } else {
        Write-Host "Version WiX différente détectée ($wixVersion). Installation/mise à jour vers v7..." -ForegroundColor Yellow
        dotnet tool update --global wix
    }
} else {
    Write-Host "Installation de WiX v7..." -ForegroundColor Yellow
    dotnet tool install --global wix
}
Write-Host "Acceptation de la licence WiX (OSMF EULA v1.1)..." -ForegroundColor Yellow
& wix eula accept wix7
if ($LASTEXITCODE -ne 0) {
    throw "Échec de l'acceptation de la licence WiX."
}

# 8. Configuration du PATH de MSYS2
Write-Host "`n=== Configuration du PATH MSYS2 ===" -ForegroundColor Cyan
Remove-LineMatching -Path $msys2Profile -Pattern '^export PATH="?\\?\s*$'
Remove-LineMatching -Path $msys2ProfileGlobal -Pattern '^export PATH="?\\?\s*$'

Add-ContentIfNotExists -Path $msys2ProfileGlobal -Line 'export PATH="/c/Program Files/dotnet:$PATH"'
Add-ContentIfNotExists -Path $msys2ProfileGlobal -Line "export PATH=\"/c/Users/$env:USERNAME/.dotnet/tools:`$PATH\""
Add-ContentIfNotExists -Path $msys2Profile -Line 'export PATH="/c/Program Files/dotnet:$PATH"'
Add-ContentIfNotExists -Path $msys2Profile -Line "export PATH=\"/c/Users/$env:USERNAME/.dotnet/tools:`$PATH\""

# 9. Vérifications finales
Write-Host "`n=== Vérifications finales ===" -ForegroundColor Cyan

Write-Host "`n--- PowerShell ---" -ForegroundColor DarkYellow
foreach ($cmd in @("dotnet --version", "wix --version")) {
    try {
        $result = Invoke-Expression $cmd 2>&1
        Write-Host "$cmd : $result" -ForegroundColor Green
    } catch {
        Write-Host "$cmd : ABSENT" -ForegroundColor Red
    }
}

Write-Host "`n--- MSYS2 MINGW64 ---" -ForegroundColor DarkYellow
$msys2Checks = @(
    @{ Cmd = "git --version"; Desc = "git" },
    @{ Cmd = "rustc --version"; Desc = "rustc" },
    @{ Cmd = "cargo --version"; Desc = "cargo" },
    @{ Cmd = "ntldd --version"; Desc = "ntldd" },
    @{ Cmd = "convert --version | head -1"; Desc = "ImageMagick" },
    @{ Cmd = "glib-compile-schemas --version"; Desc = "glib-compile-schemas" },
    @{ Cmd = "python3 --version"; Desc = "python3" },
    @{ Cmd = "wix --version"; Desc = "wix" },
    @{ Cmd = "command -v gdk-pixbuf-query-loaders"; Desc = "gdk-pixbuf-query-loaders" }
)

foreach ($check in $msys2Checks) {
    try {
        $env:MSYSTEM = "MINGW64"
        $env:MSYS2_PATH_TYPE = "minimal"
        $output = & $msys2Bash --login -c $check.Cmd 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Host "$($check.Desc) : $output" -ForegroundColor Green
        } else {
            Write-Host "$($check.Desc) : ÉCHEC ($output)" -ForegroundColor Red
        }
    } catch {
        Write-Host "$($check.Desc) : Erreur d'exécution" -ForegroundColor Red
    }
}

Write-Host "`n=== Installation terminée ===" -ForegroundColor Cyan
Write-Host "Pensez à redémarrer MSYS2 MINGW64 pour que les modifications de PATH soient prises en compte." -ForegroundColor Yellow
Write-Host "Si vous utilisez le dépôt privé premium, configurez votre clé SSH dans le home MSYS2 (~/.ssh)." -ForegroundColor Yellow
