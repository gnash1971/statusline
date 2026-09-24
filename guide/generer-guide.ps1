<#
.SYNOPSIS
    Génère les images SVG du guide utilisateur (guide.html) à partir du binaire.

.DESCRIPTION
    Chaque figure du guide est la sortie réelle de statusline.exe sur un payload
    fabriqué — jamais le compte, le cache ni le dépôt du poste : tout vient d'un
    bac isolé sous %TEMP%, sur le modèle du harnais test-statusline.ps1.

    Le binaire mesure la largeur de la console qui l'a lancé. Chaque cas est
    donc exécuté sous un conhost caché, redimensionné à la largeur voulue par
    « mode con » : c'est ce qui permet de montrer le repli à 120, 70 et 40
    colonnes quelle que soit la fenêtre d'où ce script est lancé.

    La sortie ANSI est convertie cellule par cellule en SVG. Les fonds et les
    avant-plans 24 bits sont repris tels quels — la rainure de la jauge fine
    est un fond comme un autre ; les arcs Powerline (U+E0B5, U+E0B7), les bords
    du cadre (▁ ▔), les huitièmes de la jauge fine (▏ à ▉, depuis la 2.4.0) et
    les densités de la jauge sans couleur (░ ▒ ▓ █) sont tracés en formes,
    comme Windows Terminal les trace lui-même — l'image ne dépend donc pas des
    polices du lecteur, et le texte reste du texte.

.PARAMETER Exe
    Chemin de statusline.exe. Par défaut : statusline-rs\target\release\ à côté
    de guide\ (disposition du dépôt), puis le binaire déployé deux niveaux
    au-dessus (disposition user-config), puis %USERPROFILE%\.claude\statusline.exe.

.PARAMETER Sortie
    Dossier où écrire les SVG. Par défaut, le dossier de ce script.

.PARAMETER Filtre
    Motif (-like) sur l'identifiant des cas à générer. Par défaut, tous.

.PARAMETER Lister
    Affiche chaque cas et la ligne obtenue, débarrassée de l'ANSI, sans écrire
    d'image.

.EXAMPLE
    pwsh -File guide\generer-guide.ps1
    pwsh -File guide\generer-guide.ps1 -Filtre 'largeur-*' -Lister
#>
#Requires -Version 7
[CmdletBinding()]
param(
    [string]$Exe,
    [string]$Sortie = $PSScriptRoot,
    [string]$Filtre = '*',
    [switch]$Lister
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$utf8SansBom = [System.Text.UTF8Encoding]::new($false)

# --------------------------------------------------------------------------
# Binaire et bac
# --------------------------------------------------------------------------

function Resolve-Exe {
    param([string]$Explicite)

    if ($Explicite) {
        if (Test-Path -LiteralPath $Explicite -PathType Leaf) {
            return (Resolve-Path -LiteralPath $Explicite).Path
        }
        throw "Binaire introuvable : $Explicite"
    }
    $candidats = @(
        (Join-Path $PSScriptRoot '..\statusline-rs\target\release\statusline.exe'),
        (Join-Path $PSScriptRoot '..\..\statusline.exe'),
        (Join-Path $PSScriptRoot '..\..\statusline-rs\target\release\statusline.exe'),
        (Join-Path $env:USERPROFILE '.claude\statusline.exe')
    )
    foreach ($candidat in $candidats) {
        if (Test-Path -LiteralPath $candidat -PathType Leaf) {
            return (Resolve-Path -LiteralPath $candidat).Path
        }
    }
    throw "statusline.exe introuvable : compiler le crate (cargo build --release) ou passer -Exe."
}

$bac = Join-Path ([System.IO.Path]::GetTempPath()) 'statusline-guide'
$atelier = Join-Path $bac 'atelier'

# Instant de référence, partagé par tous les cas : les remises à zéro des
# payloads, des caches et des relevés de /usage doivent coïncider.
$maintenant = [System.DateTimeOffset]::Now
$reset5h = $maintenant.AddHours(2)
$reset7j = $maintenant.AddDays(3)
$reset7j = [System.DateTimeOffset]::new($reset7j.Year, $reset7j.Month, $reset7j.Day, 9, 0, 0, $reset7j.Offset)
$r5 = $reset5h.ToUnixTimeSeconds()
$r7 = $reset7j.ToUnixTimeSeconds()
$r7Iso = $reset7j.ToString('yyyy-MM-dd''T''HH:mm:sszzz')
$ilYa1h = $maintenant.AddHours(-1).ToUnixTimeSeconds()
$passe = $maintenant.AddHours(-2).ToUnixTimeSeconds()

function Initialize-Bac {
    if (Test-Path -LiteralPath $bac) {
        Remove-Item -LiteralPath $bac -Recurse -Force
    }
    $null = New-Item -ItemType Directory -Path $bac

    # Dépôt « atelier » sur main, un sous-dossier, un chemin profond.
    $null = New-Item -ItemType Directory -Path (Join-Path $atelier '.git')
    [System.IO.File]::WriteAllText((Join-Path $atelier '.git\HEAD'), "ref: refs/heads/main`n", $utf8SansBom)
    $null = New-Item -ItemType Directory -Path (Join-Path $atelier 'rust\src\noyau\api')

    # HEAD détachée : le SHA abrégé remplace le nom de branche.
    $null = New-Item -ItemType Directory -Path (Join-Path $bac 'detache\.git')
    [System.IO.File]::WriteAllText((Join-Path $bac 'detache\.git\HEAD'), "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678`n", $utf8SansBom)

    # Hors de tout dépôt.
    $null = New-Item -ItemType Directory -Path (Join-Path $bac 'ailleurs\notes')

    # Configurations factices : l'abonnement se lit dans ~/.claude.json.
    $configs = [ordered]@{
        'pro'         = '{"oauthAccount":{"organizationType":"claude_pro"}}'
        'max5x'       = '{"oauthAccount":{"organizationType":"claude_max","organizationRateLimitTier":"default_claude_max_5x"}}'
        'max20x'      = '{"oauthAccount":{"organizationType":"claude_max","organizationRateLimitTier":"default_claude_max_20x"}}'
        'team'        = '{"oauthAccount":{"organizationType":"claude_team"}}'
        'enterprise'  = '{"oauthAccount":{"organizationType":"claude_enterprise"}}'
        'inconnu'     = '{"oauthAccount":{"organizationType":"claude_ultra_plus"}}'
        'sans-compte' = '{"numStartups":42}'
    }
    foreach ($nom in $configs.Keys) {
        [System.IO.File]::WriteAllText((Join-Path $bac "config-$nom.json"), $configs[$nom], $utf8SansBom)
    }

    # Relevés de /usage, tels que Claude Code les persiste sous
    # cachedUsageUtilization : la fenêtre globale et la fenêtre propre à Fable.
    foreach ($age in @(1, 20)) {
        $releve = [ordered]@{
            oauthAccount           = [ordered]@{
                accountUuid               = 'compte-a'
                organizationType          = 'claude_max'
                organizationRateLimitTier = 'default_claude_max_5x'
            }
            cachedUsageUtilization = [ordered]@{
                fetchedAtMs = $maintenant.AddMinutes(-$age).ToUnixTimeMilliseconds()
                accountUuid = 'compte-a'
                utilization = [ordered]@{
                    limits = @(
                        [ordered]@{ kind = 'weekly_all'; group = 'weekly'; percent = 14; severity = 'normal'; resets_at = $r7Iso; scope = $null; is_active = $false },
                        [ordered]@{ kind = 'weekly_scoped'; group = 'weekly'; percent = 28; severity = 'normal'; resets_at = $r7Iso
                            scope = [ordered]@{ model = [ordered]@{ id = $null; display_name = 'Fable' }; surface = $null }; is_active = $false }
                    )
                }
            }
        }
        [System.IO.File]::WriteAllText((Join-Path $bac "config-usage-fable-$age.json"),
            ($releve | ConvertTo-Json -Depth 8 -Compress), $utf8SansBom)
    }
}

function Get-CheminConfig {
    param([string]$Nom)
    return (Join-Path $bac "config-$Nom.json")
}

# --------------------------------------------------------------------------
# Payloads
# --------------------------------------------------------------------------

function New-Socle {
    param(
        [string]$Dossier = $atelier,
        [string]$Projet = $atelier,
        [string]$Modele = 'Opus 5',
        [string]$Id = 'claude-opus-5'
    )
    return [ordered]@{
        model     = [ordered]@{ id = $Id; display_name = $Modele }
        workspace = [ordered]@{ current_dir = $Dossier; project_dir = $Projet }
        cwd       = $Dossier
    }
}

function Avec {
    param([System.Collections.IDictionary]$Base, [System.Collections.IDictionary]$Ajouts)
    $copie = [ordered]@{}
    foreach ($cle in $Base.Keys) { $copie[$cle] = $Base[$cle] }
    foreach ($cle in $Ajouts.Keys) { $copie[$cle] = $Ajouts[$cle] }
    return $copie
}

function Mesures {
    param($Ctx = 34, $Cinq = 29, $Sept = 45)
    return [ordered]@{
        context_window = [ordered]@{ used_percentage = $Ctx }
        rate_limits    = [ordered]@{
            five_hour = [ordered]@{ used_percentage = $Cinq; resets_at = $r5 }
            seven_day = [ordered]@{ used_percentage = $Sept; resets_at = $r7 }
        }
    }
}

function Json {
    param([System.Collections.IDictionary]$Objet)
    return ($Objet | ConvertTo-Json -Depth 8 -Compress)
}

# Cache portant l'ancrage de mesure de la fenêtre de 5 heures : observée à
# $Observe % il y a une heure, à $Pourcent % maintenant.
function Ancre5h {
    param([int]$Pourcent, [int]$Observe)
    return '{"five_hour":{"used_percentage":' + $Pourcent + ',"resets_at":' + $r5 +
        ',"observe_a":' + $ilYa1h + ',"observe_pourcent":' + $Observe + '}}'
}

# --------------------------------------------------------------------------
# Exécution d'un cas sous une console cachée de largeur donnée
# --------------------------------------------------------------------------

function Invoke-Statusline {
    param(
        [string]$Payload,
        [int]$Largeur,
        [string]$Cache,
        [string]$Config,
        [switch]$SansCouleur
    )

    $etat = Join-Path $bac 'claude-code'
    if (Test-Path -LiteralPath $etat) {
        Remove-Item -LiteralPath $etat -Recurse -Force
    }
    if ($Cache) {
        $null = New-Item -ItemType Directory -Path $etat
        [System.IO.File]::WriteAllText((Join-Path $etat 'statusline-cache.json'), $Cache, $utf8SansBom)
    }

    $entree = Join-Path $bac 'payload.json'
    $resultat = Join-Path $bac 'sortie.txt'
    [System.IO.File]::WriteAllText($entree, $Payload, $utf8SansBom)
    if (Test-Path -LiteralPath $resultat) {
        Remove-Item -LiteralPath $resultat -Force
    }

    # L'environnement est hérité par le conhost, puis par le binaire.
    $env:LOCALAPPDATA = $bac
    [System.Environment]::SetEnvironmentVariable('NO_COLOR', $(if ($SansCouleur) { '1' } else { $null }))
    [System.Environment]::SetEnvironmentVariable('CLAUDE_STATUSLINE_CONFIG', $(if ($Config) { $Config } else { $null }))

    # conhost.exe explicite : la console classique, même si Windows Terminal est
    # le terminal par défaut du poste — sa fenêtre reste cachée. « mode con » y
    # fixe la largeur que le binaire mesurera ; les redirections passent les
    # octets sans transcodage.
    $commande = "mode con cols=$Largeur lines=50 >nul & `"$script:exe`" < `"$entree`" > `"$resultat`""
    $processus = Start-Process -FilePath (Join-Path $env:SystemRoot 'System32\conhost.exe') `
        -ArgumentList @('cmd.exe', '/d', '/s', '/c', "`"$commande`"") `
        -WindowStyle Hidden -Wait -PassThru
    if (-not (Test-Path -LiteralPath $resultat)) {
        throw "Aucune sortie du binaire (code $($processus.ExitCode))."
    }
    $texte = [System.Text.Encoding]::UTF8.GetString([System.IO.File]::ReadAllBytes($resultat))
    return $texte.TrimEnd("`r", "`n")
}

# --------------------------------------------------------------------------
# ANSI vers cellules
# --------------------------------------------------------------------------

# Rend, pour chaque ligne de la sortie, la liste de ses cellules : caractère,
# avant-plan, fond, gras. Seules les séquences SGR que le binaire émet sont
# interprétées : 0, 1, 22, 39, 49, 38;2;r;g;b et 48;2;r;g;b.
function ConvertFrom-Ansi {
    param([string]$Texte)

    $rangs = [System.Collections.Generic.List[object]]::new()
    foreach ($ligne in ($Texte -split "`n")) {
        $cellules = [System.Collections.Generic.List[object]]::new()
        $fg = $null
        $bg = $null
        $gras = $false
        $i = 0
        while ($i -lt $ligne.Length) {
            $c = $ligne[$i]
            if ($c -eq [char]27 -and ($i + 1) -lt $ligne.Length -and $ligne[$i + 1] -eq '[') {
                $fin = $ligne.IndexOf('m', $i)
                if ($fin -lt 0) { break }
                $parametres = $ligne.Substring($i + 2, $fin - $i - 2)
                if ($parametres -eq '') { $parametres = '0' }
                $parts = $parametres -split ';'
                $k = 0
                while ($k -lt $parts.Count) {
                    switch ($parts[$k]) {
                        '0' { $fg = $null; $bg = $null; $gras = $false }
                        '1' { $gras = $true }
                        '22' { $gras = $false }
                        '39' { $fg = $null }
                        '49' { $bg = $null }
                        '38' {
                            if (($k + 4) -lt $parts.Count -and $parts[$k + 1] -eq '2') {
                                $fg = "rgb($($parts[$k + 2]),$($parts[$k + 3]),$($parts[$k + 4]))"
                                $k += 4
                            }
                        }
                        '48' {
                            if (($k + 4) -lt $parts.Count -and $parts[$k + 1] -eq '2') {
                                $bg = "rgb($($parts[$k + 2]),$($parts[$k + 3]),$($parts[$k + 4]))"
                                $k += 4
                            }
                        }
                    }
                    $k++
                }
                $i = $fin + 1
                continue
            }
            $cellules.Add([pscustomobject]@{ Car = $c; Fg = $fg; Bg = $bg; Gras = $gras })
            $i++
        }
        $rangs.Add($cellules)
    }
    # La virgule empêche PowerShell de dérouler la liste : une sortie d'un seul
    # rang deviendrait sinon autant de rangs que de cellules.
    return , $rangs
}

function Get-TexteBrut {
    param([string]$Texte)
    return ($Texte -replace "`e\[[0-9;]*m", '')
}

# --------------------------------------------------------------------------
# Cellules vers SVG
# --------------------------------------------------------------------------

$CW = 9            # largeur d'une cellule, en pixels
$LH = 21           # hauteur d'un rang
$PAD = 14          # marge intérieure du cadre
$ECART = 12        # entre deux sorties empilées
$ETIQUETTE = 18    # hauteur d'une étiquette de sortie
$LEGENDE = 48      # hauteur de la zone des légendes
$FOND = '#0f1114'
$BORDURE = '#2a2f36'
$ENCRE_DEFAUT = '#cccccc'
$ENCRE_LEGENDE = '#8b949e'
$POLICE_MONO = "'Cascadia Mono','Consolas','DejaVu Sans Mono',monospace"
$POLICE_SANS = "'Segoe UI',system-ui,sans-serif"

$CAP_GAUCHE = [char]0xE0B7
$CAP_DROIT = [char]0xE0B5
$BORD_HAUT = [char]0x2581
$BORD_BAS = [char]0x2594
$OMBRES = @{ ([char]0x2591) = 1; ([char]0x2592) = 2; ([char]0x2593) = 3; ([char]0x2588) = 4 }
$TRAME = 3         # côté d'un carré de trame
# Huitièmes de la jauge fine, de ▏ (U+258F, un huitième) à ▉ (U+2589, sept) ;
# le plein, █, est déjà une ombre de densité 4.
$HUITIEMES = @{}
for ($k = 1; $k -le 7; $k++) { $HUITIEMES[[char](0x2590 - $k)] = $k }

# Trame une cellule à la densité demandée (1 : un carré sur quatre, 2 : un sur
# deux, 3 : trois sur quatre, 4 : pleine), comme Windows Terminal trace ░ ▒ ▓ █.
function New-Trame {
    param([double]$X, [double]$Y, [int]$Densite, [string]$Encre)

    if ($Densite -ge 4) {
        return "<rect x='$(Px $X)' y='$(Px $Y)' width='$CW' height='$LH' fill='$Encre'/>"
    }
    $carres = [System.Text.StringBuilder]::new()
    $colonnes = [int][Math]::Ceiling($CW / $TRAME)
    $rangs = [int][Math]::Ceiling($LH / $TRAME)
    for ($j = 0; $j -lt $rangs; $j++) {
        for ($i = 0; $i -lt $colonnes; $i++) {
            $phase = ($i + $j) % 4
            $plein = switch ($Densite) {
                1 { $phase -eq 0 }
                2 { ($i + $j) % 2 -eq 0 }
                default { $phase -ne 0 }
            }
            if (-not $plein) { continue }
            $w = [Math]::Min($TRAME, $CW - $i * $TRAME)
            $h = [Math]::Min($TRAME, $LH - $j * $TRAME)
            $null = $carres.Append("<rect x='$(Px ($X + $i * $TRAME))' y='$(Px ($Y + $j * $TRAME))' width='$(Px $w)' height='$(Px $h)' fill='$Encre'/>")
        }
    }
    return $carres.ToString()
}

function Px {
    param([double]$Valeur)
    return $Valeur.ToString('0.##', [System.Globalization.CultureInfo]::InvariantCulture)
}

function Esc {
    param([string]$Texte)
    return [System.Security.SecurityElement]::Escape($Texte)
}

# Écrit un rang de cellules à l'origine donnée : fonds fusionnés, puis formes
# et textes. Les textes sont regroupés par avant-plan et graisse, et calés sur
# la grille par textLength : la police du lecteur n'y change rien.
function Add-Rang {
    param(
        [System.Text.StringBuilder]$Sb,
        [System.Collections.Generic.List[object]]$Cellules,
        [double]$X0,
        [double]$Y0
    )

    # Fonds.
    $col = 0
    while ($col -lt $Cellules.Count) {
        $bg = $Cellules[$col].Bg
        $debut = $col
        while ($col -lt $Cellules.Count -and $Cellules[$col].Bg -eq $bg) { $col++ }
        if ($bg) {
            $null = $Sb.Append("<rect x='$(Px ($X0 + $debut * $CW))' y='$(Px $Y0)' width='$(Px (($col - $debut) * $CW))' height='$LH' fill='$bg'/>`n")
        }
    }

    # Formes, et textes regroupés en séquences de même style.
    $sequences = [System.Collections.Generic.List[object]]::new()
    $courante = $null
    $col = 0
    while ($col -lt $Cellules.Count) {
        $cellule = $Cellules[$col]
        $c = $cellule.Car
        $fg = if ($cellule.Fg) { $cellule.Fg } else { $ENCRE_DEFAUT }
        $x = $X0 + $col * $CW
        $forme = $null
        # Une suite de bords de même encre ne fait qu'un rectangle.
        if ($c -eq $BORD_HAUT -or $c -eq $BORD_BAS) {
            $fin = $col
            while ($fin -lt $Cellules.Count -and $Cellules[$fin].Car -eq $c -and $Cellules[$fin].Fg -eq $cellule.Fg) { $fin++ }
            $yBord = if ($c -eq $BORD_HAUT) { $Y0 + $LH * 7 / 8 } else { $Y0 }
            $null = $Sb.Append("<rect x='$(Px $x)' y='$(Px $yBord)' width='$(Px (($fin - $col) * $CW))' height='$(Px ($LH / 8))' fill='$fg'/>`n")
            $courante = $null
            $col = $fin
            continue
        }
        switch ($c) {
            $CAP_GAUCHE {
                $forme = "<path d='M $(Px ($x + $CW - 0.5)),$(Px ($Y0 + 0.6)) A $(Px ($CW - 0.8)),$(Px ($LH / 2 - 0.6)) 0 0 0 $(Px ($x + $CW - 0.5)),$(Px ($Y0 + $LH - 0.6))' fill='none' stroke='$fg' stroke-width='1.2'/>"
            }
            $CAP_DROIT {
                $forme = "<path d='M $(Px ($x + 0.5)),$(Px ($Y0 + 0.6)) A $(Px ($CW - 0.8)),$(Px ($LH / 2 - 0.6)) 0 0 1 $(Px ($x + 0.5)),$(Px ($Y0 + $LH - 0.6))' fill='none' stroke='$fg' stroke-width='1.2'/>"
            }
            default {
                # Les ombres sont tramées comme le terminal les trame : des carrés
                # de trois pixels, un sur quatre, un sur deux, trois sur quatre.
                if ($OMBRES.ContainsKey($c)) {
                    $forme = New-Trame -X $x -Y $Y0 -Densite $OMBRES[$c] -Encre $fg
                }
                # Un huitième : une barre collée à gauche, pleine hauteur, sur
                # le fond de la cellule — la rainure.
                elseif ($HUITIEMES.ContainsKey($c)) {
                    $forme = "<rect x='$(Px $x)' y='$(Px $Y0)' width='$(Px ($CW * $HUITIEMES[$c] / 8))' height='$LH' fill='$fg'/>"
                }
            }
        }
        if ($forme) {
            $null = $Sb.Append($forme + "`n")
            $courante = $null
            $col++
            continue
        }
        if ($null -eq $courante -or $courante.Fg -ne $fg -or $courante.Gras -ne $cellule.Gras) {
            $courante = @{ Debut = $col; Fg = $fg; Gras = $cellule.Gras; Texte = [System.Text.StringBuilder]::new() }
            $sequences.Add($courante)
        }
        $null = $courante.Texte.Append($c)
        $col++
    }

    foreach ($sequence in $sequences) {
        $contenu = $sequence.Texte.ToString()
        if ($contenu.Trim().Length -eq 0) { continue }
        $graisse = if ($sequence.Gras) { " font-weight='bold'" } else { '' }
        $null = $Sb.Append("<text x='$(Px ($X0 + $sequence.Debut * $CW))' y='$(Px ($Y0 + $LH / 2))' textLength='$(Px ($contenu.Length * $CW))' lengthAdjust='spacingAndGlyphs' fill='$($sequence.Fg)'$graisse xml:space='preserve'>$(Esc $contenu)</text>`n")
    }
}

# Compose l'image d'un cas : les sorties empilées, chacune sous son étiquette,
# dans un cadre de la largeur de la console (-Colonnes) ou du contenu.
function New-Svg {
    param(
        [string]$Id,
        [object[]]$Sorties,      # @{ Rangs; Etiquette; Brut }
        [int]$Colonnes,
        [string[]]$Legendes
    )

    $largeurTexte = 0
    $hauteur = $PAD
    foreach ($figure in $Sorties) {
        foreach ($rang in $figure.Rangs) {
            if ($rang.Count -gt $largeurTexte) { $largeurTexte = $rang.Count }
        }
        if ($figure.Etiquette) { $hauteur += $ETIQUETTE }
        $hauteur += $figure.Rangs.Count * $LH + $ECART
    }
    $hauteur += $PAD - $ECART
    if ($Legendes) { $hauteur += $LEGENDE }
    $colonnesCadre = if ($Colonnes -gt 0) { $Colonnes } else { $largeurTexte + 1 }
    $largeur = $PAD * 2 + $colonnesCadre * $CW

    $sb = [System.Text.StringBuilder]::new()
    $null = $sb.Append("<?xml version='1.0' encoding='UTF-8'?>`n")
    $null = $sb.Append("<svg xmlns='http://www.w3.org/2000/svg' width='$largeur' height='$hauteur' viewBox='0 0 $largeur $hauteur' role='img' aria-labelledby='titre'>`n")
    $null = $sb.Append("<title id='titre'>$(Esc "statusline — $Id")</title>`n")
    $null = $sb.Append("<desc>$(Esc (($Sorties | ForEach-Object { $_.Brut }) -join "`n"))</desc>`n")
    $null = $sb.Append("<style>text{font-family:$POLICE_MONO;font-size:15px;dominant-baseline:central;white-space:pre}.l{font-family:$POLICE_SANS;font-size:12px;fill:$ENCRE_LEGENDE;dominant-baseline:auto}</style>`n")
    $null = $sb.Append("<rect x='0.5' y='0.5' width='$($largeur - 1)' height='$($hauteur - 1)' rx='8' fill='$FOND' stroke='$BORDURE'/>`n")

    $y = $PAD
    $rangPilule = $null
    $yLegende = 0
    foreach ($figure in $Sorties) {
        if ($figure.Etiquette) {
            $null = $sb.Append("<text class='l' x='$PAD' y='$(Px ($y + 12))'>$(Esc $figure.Etiquette)</text>`n")
            $y += $ETIQUETTE
        }
        foreach ($rang in $figure.Rangs) {
            Add-Rang -Sb $sb -Cellules $rang -X0 $PAD -Y0 $y
            if ($null -eq $rangPilule -and @($rang | Where-Object { $_.Bg }).Count -gt 0) {
                $rangPilule = $rang
                $yLegende = $y + $LH * 2
            }
            $y += $LH
        }
        $y += $ECART
    }

    # Légendes : une accolade sous chaque tranche du premier rang coloré — un
    # compartiment, ou une mesure depuis la 2.4.0. Une tranche commence à la
    # première cellule de fond, puis à chaque jonction (l'arc porte le fond de
    # la tranche qu'il ouvre). Les plages de fond contiguës ne suffisent plus :
    # la rainure d'une jauge change de fond au milieu d'une mesure, et deux
    # mesures calmes partagent la même ardoise.
    if ($Legendes -and $rangPilule) {
        $plages = [System.Collections.Generic.List[object]]::new()
        $debut = -1
        $fin = -1
        for ($col = 0; $col -lt $rangPilule.Count; $col++) {
            $cellule = $rangPilule[$col]
            if (-not $cellule.Bg) { continue }
            if ($debut -ge 0 -and $cellule.Car -eq $CAP_DROIT) {
                $plages.Add(@{ Debut = $debut; Fin = $col })
                $debut = $col
            }
            elseif ($debut -lt 0) { $debut = $col }
            $fin = $col + 1
        }
        if ($debut -ge 0) { $plages.Add(@{ Debut = $debut; Fin = $fin }) }
        for ($i = 0; $i -lt [Math]::Min($plages.Count, $Legendes.Count); $i++) {
            $x1 = $PAD + $plages[$i].Debut * $CW + 2
            $x2 = $PAD + $plages[$i].Fin * $CW - 2
            $yb = $yLegende + 6
            $null = $sb.Append("<path d='M $(Px $x1),$(Px $yb) v6 H $(Px $x2) v-6' fill='none' stroke='$ENCRE_LEGENDE'/>`n")
            $null = $sb.Append("<text class='l' x='$(Px (($x1 + $x2) / 2))' y='$(Px ($yb + 24))' text-anchor='middle'>$(Esc $Legendes[$i])</text>`n")
        }
    }

    $null = $sb.Append("</svg>`n")
    return $sb.ToString()
}

# --------------------------------------------------------------------------
# Les cas
# --------------------------------------------------------------------------

function Get-Cas {
    $unNiveau = Join-Path $atelier 'rust'
    $sousDossier = Join-Path $atelier 'rust\src'
    $profond = Join-Path $atelier 'rust\src\noyau\api'
    $detache = Join-Path $bac 'detache'
    $ailleurs = Join-Path $bac 'ailleurs\notes'

    $identite = New-Socle
    $courant = Avec (New-Socle -Dossier $sousDossier) ([ordered]@{ effort = [ordered]@{ level = 'high' } })
    $complet = Avec $courant (Mesures 34 20 45)
    $ancrage = Ancre5h 20 8

    $fable = New-Socle -Dossier $sousDossier -Modele 'Fable 5.1' -Id 'claude-fable-5-1'

    # La jauge fine depuis la 2.4.0 : seize huitièmes, au moins un dès 1 %, le
    # plein à 100 % seulement ; 78 passe le seuil d'alerte hebdomadaire (75),
    # 97 le seuil critique (85).
    $jauge = foreach ($p in @(0, 3, 20, 45, 56, 78, 97, 100)) {
        @{ Etiquette = "$p %"; Payload = (Json (Avec $identite ([ordered]@{ rate_limits = [ordered]@{ seven_day = [ordered]@{ used_percentage = $p; resets_at = $r7 } } }))) }
    }

    return @(
        @{ Id = 'anatomie'; Largeur = 120
           Legendes = @('abonnement', 'modèle', 'emplacement', 'contexte', 'fenêtre de 5 heures', 'fenêtre de 7 jours')
           Sorties = @(@{ Payload = (Json $complet); Cache = $ancrage }) }

        @{ Id = 'tete-abonnements'; Largeur = 120; Sorties = @(
            @{ Etiquette = 'claude_pro'; Payload = (Json $identite); Config = 'pro' }
            @{ Etiquette = 'claude_max, palier 5x'; Payload = (Json $identite); Config = 'max5x' }
            @{ Etiquette = 'claude_max, palier 20x'; Payload = (Json $identite); Config = 'max20x' }
            @{ Etiquette = 'claude_team'; Payload = (Json $identite); Config = 'team' }
            @{ Etiquette = 'claude_enterprise'; Payload = (Json $identite); Config = 'enterprise' }
            @{ Etiquette = 'type inconnu de la table : claude_ultra_plus'; Payload = (Json $identite); Config = 'inconnu' }
            @{ Etiquette = 'aucun compte lisible dans ~/.claude.json'; Payload = (Json $identite); Config = 'sans-compte' }
        ) }

        @{ Id = 'modeles'; Largeur = 120; Sorties = @(
            @{ Etiquette = 'claude-opus-5'; Payload = (Json $identite) }
            @{ Etiquette = 'claude-sonnet-5'; Payload = (Json (New-Socle -Modele 'Sonnet 5' -Id 'claude-sonnet-5')) }
            @{ Etiquette = 'claude-haiku-4-5-20251001'; Payload = (Json (New-Socle -Modele 'Haiku 4.5' -Id 'claude-haiku-4-5-20251001')) }
            @{ Etiquette = 'claude-fable-5-1'; Payload = (Json (New-Socle -Modele 'Fable 5.1' -Id 'claude-fable-5-1')) }
        ) }

        @{ Id = 'fenetre-1m'; Largeur = 120; Sorties = @(
            @{ Etiquette = 'context_window_size = 200 000'; Payload = (Json (Avec $identite ([ordered]@{ context_window = [ordered]@{ used_percentage = 61; context_window_size = 200000 } }))) }
            @{ Etiquette = 'context_window_size = 1 000 000 : la session tourne en contexte étendu'; Payload = (Json (Avec $identite ([ordered]@{ context_window = [ordered]@{ used_percentage = 12; context_window_size = 1000000 } }))) }
        ) }

        @{ Id = 'efforts'; Largeur = 120; Sorties = @(foreach ($niveau in @('low', 'medium', 'high', 'xhigh', 'max')) {
            @{ Etiquette = "effort.level = $niveau"; Payload = (Json (Avec $identite ([ordered]@{ effort = [ordered]@{ level = $niveau } }))) }
        }) }

        @{ Id = 'modes'; Largeur = 120; Sorties = @(
            @{ Etiquette = 'fast_mode = true'; Payload = (Json (Avec $identite ([ordered]@{ effort = [ordered]@{ level = 'high' }; fast_mode = $true }))) }
            @{ Etiquette = 'thinking.enabled = false'; Payload = (Json (Avec $identite ([ordered]@{ effort = [ordered]@{ level = 'high' }; thinking = [ordered]@{ enabled = $false } }))) }
            @{ Etiquette = 'tout à la fois : contexte étendu, effort, mode rapide, réflexion coupée'
               Payload = (Json (Avec $identite ([ordered]@{ context_window = [ordered]@{ used_percentage = 12; context_window_size = 1000000 }; effort = [ordered]@{ level = 'xhigh' }; fast_mode = $true; thinking = [ordered]@{ enabled = $false } }))) }
        ) }

        @{ Id = 'lieux'; Largeur = 120; Sorties = @(
            @{ Etiquette = 'à la racine du projet, sur main'; Payload = (Json $identite) }
            @{ Etiquette = 'un niveau sous la racine : le chemin depuis la racine du projet'; Payload = (Json (New-Socle -Dossier $unNiveau)) }
            @{ Etiquette = 'plus bas : seule la feuille reste, derrière « … »'; Payload = (Json (New-Socle -Dossier $profond)) }
            @{ Etiquette = 'HEAD détachée : le SHA abrégé remplace la branche'; Payload = (Json (New-Socle -Dossier $detache -Projet $detache)) }
            @{ Etiquette = 'session --worktree : la branche vient du payload'; Payload = (Json (Avec (New-Socle -Dossier $sousDossier) ([ordered]@{ worktree = [ordered]@{ branch = 'feature/paie' } }))) }
            @{ Etiquette = 'hors du projet, hors de tout dépôt : la feuille seule'; Payload = (Json (New-Socle -Dossier $ailleurs)) }
        ) }

        @{ Id = 'jauge'; Largeur = 120; Sorties = $jauge }

        @{ Id = '5h-formes'; Largeur = 120; Sorties = @(
            @{ Etiquette = 'mesure fraîche : jauge, valeur, heure de remise à zéro'; Payload = (Json (Avec $identite (Mesures 34 29 45))) }
            @{ Etiquette = 'rythme mesuré depuis une heure : projection à la remise à zéro'; Payload = (Json (Avec $identite (Mesures 34 20 45))); Cache = (Ancre5h 20 8) }
            @{ Etiquette = 'la projection franchit le seuil : elle prend la couleur du palier, la fenêtre son fond et son cadre'; Payload = (Json (Avec $identite (Mesures 34 40 45))); Cache = (Ancre5h 40 18) }
            @{ Etiquette = 'le plafond arrivera avant la remise à zéro : heure d''épuisement'; Payload = (Json (Avec $identite (Mesures 34 40 45))); Cache = (Ancre5h 40 5) }
        ) }

        @{ Id = '7j-paliers'; Largeur = 120; Sorties = @(
            @{ Etiquette = '45 % : sous les seuils, pas d''échéance'; Payload = (Json (Avec $identite (Mesures 34 29 45))) }
            @{ Etiquette = '78 % : alerte, la remise à zéro apparaît'; Payload = (Json (Avec $identite (Mesures 34 29 78))) }
            @{ Etiquette = '88 % : critique'; Payload = (Json (Avec $identite (Mesures 34 29 88))) }
        ) }

        @{ Id = '7j-marqueurs'; Largeur = 120; Sorties = @(
            @{ Etiquette = '~ : valeurs du cache, avant la première réponse de la session'
               Payload = (Json (Avec $identite ([ordered]@{ context_window = [ordered]@{ used_percentage = 34 } })))
               Cache = ('{"five_hour":{"used_percentage":29,"resets_at":' + $r5 + '},"seven_day":{"used_percentage":45,"resets_at":' + $r7 + '}}') }
            @{ Etiquette = 'fenêtre propre au modèle, relevée par /usage (28 %), à la place de la globale (14 %)'
               Payload = (Json (Avec $fable (Mesures 34 29 14))); Config = 'usage-fable-1' }
            @{ Etiquette = '≈ : la globale a pris 6 points depuis le relevé, la fenêtre du modèle est avancée d''autant'
               Payload = (Json (Avec $fable (Mesures 34 29 20))); Config = 'usage-fable-20' }
        ) }

        @{ Id = '7j-selon-modele'; Largeur = 120; Sorties = @(
            @{ Etiquette = 'Opus 5 à 55 % : seuils 75 / 85'; Payload = (Json (Avec (New-Socle -Dossier $sousDossier) (Mesures 34 29 55))) }
            @{ Etiquette = 'Fable 5.1 à 55 % : seuils abaissés à 50 / 70, alerte'; Payload = (Json (Avec $fable (Mesures 34 29 55))) }
            @{ Etiquette = 'Fable 5.1 à 72 % : critique'; Payload = (Json (Avec $fable (Mesures 34 29 72))) }
        ) }

        @{ Id = 'paliers'; Largeur = 120; Sorties = @(
            @{ Etiquette = 'contexte à 34 % : ardoise'; Payload = (Json (Avec $courant (Mesures 34 29 45))) }
            @{ Etiquette = 'contexte à 82 % : alerte, le contexte seul passe à l''ambre, son cadre aussi'; Payload = (Json (Avec $courant (Mesures 82 29 45))) }
            @{ Etiquette = 'contexte à 93 % : critique, rouge sombre, cadre corail, valeur en gras'; Payload = (Json (Avec $courant (Mesures 93 29 45))) }
            @{ Etiquette = 'contexte à 93 %, 7 jours à 78 % : chaque mesure son palier ; l''arc entre deux mesures prend le pire'; Payload = (Json (Avec $courant (Mesures 93 29 78))) }
        ) }

        @{ Id = 'largeur-120'; Largeur = 120; Cadre = 'console'; Sorties = @(@{ Payload = (Json $complet); Cache = $ancrage }) }
        # Largeurs recalées le 23/09/2026 : sans la version et avec le chemin
        # replié sur sa feuille, la capsule a perdu une vingtaine de colonnes et
        # tenait entière à 90 ; 70 et 40 montrent les deux coupes.
        @{ Id = 'largeur-70'; Largeur = 70; Cadre = 'console'; Sorties = @(@{ Payload = (Json $complet); Cache = $ancrage }) }
        @{ Id = 'largeur-40'; Largeur = 40; Cadre = 'console'; Sorties = @(@{ Payload = (Json $complet); Cache = $ancrage }) }

        @{ Id = 'sans-couleur'; Largeur = 120; Sorties = @(@{ Payload = (Json $complet); Cache = $ancrage; SansCouleur = $true }) }

        @{ Id = 'degrades'; Largeur = 120; Sorties = @(
            @{ Etiquette = 'payload réduit au nom du modèle, sans compte lisible'; Payload = '{"model":{"display_name":"Opus 5"}}'; Config = 'sans-compte' }
            @{ Etiquette = 'payload vide : le repli « Claude », jamais une ligne blanche'; Payload = '{}'; Config = 'sans-compte' }
            @{ Etiquette = 'ni rate_limits, ni cache valide : les fenêtres se retirent'
               Payload = (Json (Avec $courant ([ordered]@{ context_window = [ordered]@{ used_percentage = 34 } })))
               Cache = ('{"five_hour":{"used_percentage":29,"resets_at":' + $passe + '}}') }
        ) }
    )
}

# --------------------------------------------------------------------------
# Programme principal
# --------------------------------------------------------------------------

$script:exe = Resolve-Exe -Explicite $Exe
Write-Host "Binaire : $script:exe"
Write-Host "Bac     : $bac"
if (-not $Lister) {
    $null = New-Item -ItemType Directory -Path $Sortie -Force
    Write-Host "Sortie  : $Sortie"
}
Write-Host ''

$environnementInitial = @{
    LOCALAPPDATA               = $env:LOCALAPPDATA
    NO_COLOR                   = $env:NO_COLOR
    CLAUDE_STATUSLINE_CONFIG   = $env:CLAUDE_STATUSLINE_CONFIG
}

try {
    Initialize-Bac
    $total = 0
    foreach ($cas in (Get-Cas | Where-Object { $_.Id -like $Filtre })) {
        $sorties = foreach ($spec in $cas.Sorties) {
            $config = if ($spec.ContainsKey('Config')) { Get-CheminConfig $spec.Config } else { Get-CheminConfig 'max5x' }
            $texte = Invoke-Statusline -Payload $spec.Payload -Largeur $cas.Largeur `
                -Cache $(if ($spec.ContainsKey('Cache')) { $spec.Cache } else { '' }) `
                -Config $config `
                -SansCouleur:($spec.ContainsKey('SansCouleur') -and $spec.SansCouleur)
            @{
                Rangs     = (ConvertFrom-Ansi $texte)
                Etiquette = $(if ($spec.ContainsKey('Etiquette')) { $spec.Etiquette } else { '' })
                Brut      = (Get-TexteBrut $texte)
            }
        }
        $sorties = @($sorties)
        if ($Lister) {
            Write-Host "== $($cas.Id) ($($cas.Largeur) colonnes)"
            foreach ($figure in $sorties) {
                if ($figure.Etiquette) { Write-Host "   [$($figure.Etiquette)]" }
                foreach ($ligne in ($figure.Brut -split "`n")) { Write-Host "   $ligne" }
            }
            continue
        }
        $colonnes = if ($cas.ContainsKey('Cadre') -and $cas.Cadre -eq 'console') { $cas.Largeur } else { 0 }
        $legendes = if ($cas.ContainsKey('Legendes')) { $cas.Legendes } else { @() }
        $svg = New-Svg -Id $cas.Id -Sorties $sorties -Colonnes $colonnes -Legendes $legendes
        $chemin = Join-Path $Sortie "$($cas.Id).svg"
        [System.IO.File]::WriteAllText($chemin, $svg, $utf8SansBom)
        $total++
        Write-Host ("{0,-18} {1,2} sortie(s)  {2,7:N0} o" -f $cas.Id, $sorties.Count, (Get-Item -LiteralPath $chemin).Length)
    }
    if (-not $Lister) {
        Write-Host ''
        Write-Host "$total image(s) écrite(s)."
    }
} finally {
    foreach ($nom in $environnementInitial.Keys) {
        [System.Environment]::SetEnvironmentVariable($nom, $environnementInitial[$nom])
    }
}
