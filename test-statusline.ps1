<#
.SYNOPSIS
    Harnais de non-régression de statusline.ps1.

.DESCRIPTION
    Soumet une série de payloads au script de ligne de statut et observe ce
    qu'un appelant peut réellement constater : la ligne écrite sur la sortie
    standard, le code de sortie, et l'état du cache après coup.

    Le code de sortie est observé au même titre que la ligne, parce que Claude
    Code traite les deux de la même manière : sortie blanche ou code non nul,
    la ligne affichée est effacée. Un cas dont le code n'est pas 0 est signalé
    en mode affichage, et compté en divergence en mode comparaison.

    Deux usages :

      - sans -Reference, le harnais affiche le comportement de -Candidat. Utile
        pour relire d'un coup d'œil ce que produit chaque cas ;
      - avec -Reference, il exécute les deux scripts sur les mêmes cas et
        signale toute divergence. C'est le mode non-régression : refactoriser
        sans changer le comportement doit laisser sortir « aucune divergence ».

    L'exécution est isolée. %LOCALAPPDATA% est redirigé vers un dossier
    temporaire, si bien que le cache réel de l'utilisateur n'est ni lu ni
    écrit, et que chaque cas part d'un état de cache maîtrisé. NO_COLOR est
    positionné par cas, ce qui permet de couvrir la coloration alors que
    Claude Code définit cette variable pour les processus qu'il lance.

    Deux exceptions à cette isolation, de même nature : le segment de version
    lit le binaire réellement installé sur le poste, et le segment d'abonnement
    la configuration réelle du compte. Les cas ordinaires affichent donc la
    version du jour et l'abonnement du poste. Ce n'est pas gênant en mode
    comparaison — les deux scripts lisent les mêmes fichiers — mais la sortie du
    mode affichage suit les mises à jour de Claude Code et un changement
    d'abonnement. Les cas dont le nom commence par « binaire » ou « abonnement »
    détournent CLAUDE_STATUSLINE_BINAIRE et CLAUDE_STATUSLINE_CONFIG pour
    couvrir, eux, les chemins de repli sans rien supposer du poste.

    Sortie 1 en cas de divergence, pour un usage en contrôle automatique.

.EXAMPLE
    pwsh -File .claude\user-config\test-statusline.ps1
    Affiche le comportement de la copie projet.

.EXAMPLE
    pwsh -File .claude\user-config\test-statusline.ps1 -Reference ancien.ps1
    Compare la copie projet à une version antérieure.
#>
[CmdletBinding()]
param(
    # Script à observer. Par défaut, la copie versionnée voisine.
    [string]$Candidat = (Join-Path $PSScriptRoot "statusline.ps1"),

    # Version de référence. Fournie, le harnais bascule en mode comparaison.
    [string]$Reference,

    # En mode comparaison, saute les cas colorés et les compte à part — 19/09/2026.
    #
    # L'oracle statusline.ps1 est gelé depuis la capsule (chantier
    # CHANTIER-statusline-capsule.md, Q2) : la sortie colorée de statusline.exe a
    # changé de forme — deux lignes, des fonds, des caps — et le script ne la
    # décrit plus. Sous NO_COLOR, en revanche, l'exe rend octet pour octet la
    # ligne d'avant : c'est ce que ce drapeau laisse au harnais à vérifier, à
    # zéro divergence, pour que la comparaison reste un contrôle automatique.
    # Les cas colorés sont couverts par les tests du crate, attendus figés.
    [switch]$IgnorerCouleur
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# --------------------------------------------------------------------------
# Bac à sable
# --------------------------------------------------------------------------

# Prépare un dossier isolé : faux %LOCALAPPDATA% et dépôts Git factices.
#
# Les dépôts sont fabriqués à la main plutôt qu'avec « git init » : le harnais
# ne suppose pas git installé, et Get-BrancheGit ne lit de toute façon que
# « .git/HEAD ».
function Initialize-Bac {
    param(
        [string]$Racine,
        # Instant de référence, partagé avec Get-Cas : les remises à zéro des
        # relevés de /usage fabriqués ici doivent coïncider avec celles des
        # payloads et des caches injectés par les cas.
        [System.DateTimeOffset]$Instant = [System.DateTimeOffset]::UtcNow
    )

    if (Test-Path -LiteralPath $Racine) {
        Remove-Item -LiteralPath $Racine -Recurse -Force
    }
    New-Item -ItemType Directory -Path $Racine -Force | Out-Null

    # Dépôt ordinaire, sur une branche nommée.
    $normal = Join-Path $Racine "depot\.git"
    New-Item -ItemType Directory -Path $normal -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $normal "HEAD") `
        -Value "ref: refs/heads/main" -Encoding UTF8

    # Sous-dossier du dépôt : vérifie la remontée vers la racine.
    New-Item -ItemType Directory -Path (Join-Path $Racine "depot\src\api") -Force |
        Out-Null

    # HEAD détachée : le SHA abrégé doit remplacer le nom de branche.
    $detache = Join-Path $Racine "detache\.git"
    New-Item -ItemType Directory -Path $detache -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $detache "HEAD") `
        -Value "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678" -Encoding UTF8

    # « .git » fichier pointant ailleurs, comme le posent les worktrees liés.
    $vrai = Join-Path $Racine "ailleurs"
    New-Item -ItemType Directory -Path $vrai -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $vrai "HEAD") `
        -Value "ref: refs/heads/feature/paie" -Encoding UTF8
    New-Item -ItemType Directory -Path (Join-Path $Racine "pointeur") -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $Racine "pointeur\.git") `
        -Value "gitdir: $vrai" -Encoding UTF8

    # Dossier accentué : le workspace est francophone de bout en bout.
    New-Item -ItemType Directory -Path (Join-Path $Racine "données\été") -Force |
        Out-Null

    # Faux binaire : un fichier quelconque, donc sans métadonnées de version.
    # Couvre le cas d'une installation dont l'exécutable n'en porte pas.
    Set-Content -LiteralPath (Join-Path $Racine "sans-version.exe") `
        -Value "ceci n'est pas un exécutable" -Encoding UTF8

    # Configurations factices pour le segment d'abonnement. Aucune ne porte de
    # jeton : le champ lu vit dans « ~/.claude.json », jamais dans le fichier
    # d'identifiants — voir Get-Abonnement.
    #
    # Le compte réel du poste ne couvre qu'un seul de ces cas, et il change avec
    # l'abonnement de l'utilisateur : ces fichiers-ci exercent les autres sans
    # rien en supposer.
    Set-Content -LiteralPath (Join-Path $Racine "config-max.json") -Encoding UTF8 `
        -Value '{"oauthAccount":{"organizationType":"claude_max","organizationRateLimitTier":"default_claude_max_20x"}}'

    Set-Content -LiteralPath (Join-Path $Racine "config-inconnu.json") -Encoding UTF8 `
        -Value '{"oauthAccount":{"organizationType":"claude_ultra_plus"}}'

    Set-Content -LiteralPath (Join-Path $Racine "config-sans-compte.json") -Encoding UTF8 `
        -Value '{"numStartups":42}'

    Set-Content -LiteralPath (Join-Path $Racine "config-tronquee.json") -Encoding UTF8 `
        -Value '{"oauthAccount":{"organizat'

    # Relevés de /usage, tels que Claude Code les persiste dans ~/.claude.json
    # sous cachedUsageUtilization — 03/09/2026 au soir. Le compte réel du poste
    # n'en montre qu'un état, qui change à chaque ouverture de /usage : ces
    # fichiers-ci fixent les autres. Tous portent un abonnement Max 5x, pour
    # que les cas qui les détournent ne dépendent pas non plus du compte réel.
    function Barre {
        param([string]$Genre, $Pourcent, [string]$Reset, [string]$Modele)

        # Sans remise à zéro, l'entrée en porte une nulle, comme l'API le fait
        # pour une fenêtre pas encore entamée — 04/09/2026.
        $barre = [ordered]@{
            kind = $Genre; group = "weekly"; percent = $Pourcent
            severity = "normal"; resets_at = if ($Reset) { $Reset } else { $null }
            scope = $null; is_active = $false
        }
        if ($Modele) {
            $barre.scope = [ordered]@{
                model = [ordered]@{ id = $null; display_name = $Modele }
                surface = $null
            }
        }
        return $barre
    }

    function Usage {
        param([long]$FetchedAtMs, $Limites, [string]$CompteReleve = "compte-a")

        $config = [ordered]@{
            oauthAccount = [ordered]@{
                accountUuid               = "compte-a"
                organizationType          = "claude_max"
                organizationRateLimitTier = "default_claude_max_5x"
            }
            cachedUsageUtilization = [ordered]@{
                fetchedAtMs = $FetchedAtMs
                accountUuid = $CompteReleve
                utilization = [ordered]@{ limits = $Limites }
            }
        }
        return ($config | ConvertTo-Json -Depth 8 -Compress)
    }

    $resetHebdo = $Instant.AddDays(4).ToString("o")
    $resetPasse = $Instant.AddHours(-1).ToString("o")
    $recent = $Instant.AddMinutes(-1).ToUnixTimeMilliseconds()
    $ancien = $Instant.AddHours(-2).ToUnixTimeMilliseconds()
    $globale = Barre "weekly_all" 14 $resetHebdo

    $releves = @{
        # Le relevé du poste tel quel : 14 % pour tous les modèles, 28 % pour
        # Fable, une minute d'âge.
        "config-usage-fable.json"         = Usage $recent @($globale, (Barre "weekly_scoped" 28 $resetHebdo "Fable"))
        # Fenêtre Fable au-dessus du seuil hebdomadaire ordinaire.
        "config-usage-fable-alerte.json"  = Usage $recent @($globale, (Barre "weekly_scoped" 80 $resetHebdo "Fable"))
        # Relevé vieux de deux heures : au-delà de l'heure tolérée.
        "config-usage-fable-ancien.json"  = Usage $ancien @($globale, (Barre "weekly_scoped" 28 $resetHebdo "Fable"))
        # Fenêtre Fable déjà remise à zéro.
        "config-usage-fable-expiree.json" = Usage $recent @($globale, (Barre "weekly_scoped" 28 $resetPasse "Fable"))
        # Fenêtre Fable pas encore entamée, le lendemain d'une remise à zéro :
        # 0 %, sans échéance — le relevé du poste le 04/09/2026 au soir, où la
        # globale était à 2 %.
        "config-usage-fable-vierge.json"  = Usage $recent @((Barre "weekly_all" 2 $resetHebdo), (Barre "weekly_scoped" 0 "" "Fable"))
        # Fenêtre propre à un autre modèle.
        "config-usage-sonnet.json"        = Usage $recent @($globale, (Barre "weekly_scoped" 28 $resetHebdo "Sonnet"))
        # « limits » qui n'est pas un tableau.
        "config-usage-invalide.json"      = Usage $recent 28
        # Relevé d'un autre compte que celui connecté.
        "config-usage-autre-compte.json"  = Usage $recent @($globale, (Barre "weekly_scoped" 28 $resetHebdo "Fable")) -CompteReleve "compte-b"
        # Le relevé porte la même globale que le payload de deux cas
        # d'attribution — 60 % en alerte, 30 % sans — pour que la dérive du
        # 05/09/2026 n'y avance pas la fenêtre du modèle.
        "config-usage-fable-globale-60.json" = Usage $recent @((Barre "weekly_all" 60 $resetHebdo), (Barre "weekly_scoped" 28 $resetHebdo "Fable"))
        "config-usage-fable-globale-30.json" = Usage $recent @((Barre "weekly_all" 30 $resetHebdo), (Barre "weekly_scoped" 28 $resetHebdo "Fable"))
        # Relevés sans point de référence pour la dérive : pas d'entrée
        # globale, ou une globale sans échéance.
        "config-usage-fable-sans-globale.json" = Usage $recent @((Barre "session" 36 $resetHebdo), (Barre "weekly_scoped" 28 $resetHebdo "Fable"))
        "config-usage-fable-globale-sans-echeance.json" = Usage $recent @((Barre "weekly_all" 14 ""), (Barre "weekly_scoped" 28 $resetHebdo "Fable"))
    }
    foreach ($nom in $releves.Keys) {
        Set-Content -LiteralPath (Join-Path $Racine $nom) -Encoding UTF8 -Value $releves[$nom]
    }
}

# --------------------------------------------------------------------------
# Construction des cas
# --------------------------------------------------------------------------

# Sérialise un payload de session en JSON compact.
function New-Payload {
    param([hashtable]$Champs)

    return ($Champs | ConvertTo-Json -Depth 8 -Compress)
}

# Décrit un cas : nom, payload, cache initial, coloration attendue, et
# éventuels détournements du binaire dont la version est lue et de la
# configuration où se lit l'abonnement.
#
# -Binaire et -Config vides laissent le script chercher l'installation et le
# compte réels du poste, ce qui est le cas de la grande majorité des cas.
function New-Cas {
    param(
        [string]$Nom,
        [string]$Payload,
        [string]$Cache,
        [switch]$AvecCouleur,
        [string]$Binaire,
        [string]$Config
    )

    return [pscustomobject]@{
        Nom     = $Nom
        Payload = $Payload
        Cache   = $Cache
        Couleur = [bool]$AvecCouleur
        Binaire = $Binaire
        Config  = $Config
    }
}

# Construit la liste des cas. Les horodatages sont calculés une seule fois :
# les deux scripts comparés doivent recevoir exactement les mêmes entrées.
function Get-Cas {
    param(
        [string]$Racine,
        # Le même instant que celui d'Initialize-Bac : voir sa raison là-bas.
        [System.DateTimeOffset]$Instant = [System.DateTimeOffset]::UtcNow
    )

    $projet = Join-Path $Racine "depot"
    $maintenant = $Instant
    $dans3h = $maintenant.AddHours(3).ToUnixTimeSeconds()
    $futurLointain = $maintenant.AddDays(4).ToUnixTimeSeconds()
    $passe = $maintenant.AddHours(-2).ToUnixTimeSeconds()
    $iso = $maintenant.AddHours(3).ToString("o")

    # Instants de calage du rythme. Les cas sont dimensionnés pour que leur
    # résultat arrondi ne bascule pas sous la dérive de quelques secondes qui
    # sépare la construction des cas de leur exécution.
    $dans2h = $maintenant.AddHours(2).ToUnixTimeSeconds()
    # Encadrent la borne d'extrapolation pour un ancrage vieux d'une heure :
    # cinq heures restantes donnent un rapport de 5, sept heures un rapport
    # de 7, de part et d'autre de $RatioMaxExtrapolation. L'écart à la borne
    # est assez large pour qu'aucune dérive de quelques secondes ne le franchisse.
    $dans5h = $maintenant.AddHours(5).ToUnixTimeSeconds()
    $dans7h = $maintenant.AddHours(7).ToUnixTimeSeconds()
    $dans10min = $maintenant.AddMinutes(10).ToUnixTimeSeconds()
    $dans2min = $maintenant.AddMinutes(2).ToUnixTimeSeconds()
    $ilYa1h = $maintenant.AddHours(-1).ToUnixTimeSeconds()
    $ilYa5min = $maintenant.AddMinutes(-5).ToUnixTimeSeconds()
    $ilYa2j = $maintenant.AddDays(-2).ToUnixTimeSeconds()

    # Cache au format courant : une entrée par fenêtre, sous la clé du payload.
    $cacheValide = '{"five_hour":{"used_percentage":11,"resets_at":' +
        $dans3h + '}}'
    $cachePerime = '{"five_hour":{"used_percentage":11,"resets_at":' +
        $passe + '}}'
    $cacheDeuxFenetres = '{"five_hour":{"used_percentage":11,"resets_at":' +
        $dans3h + '},"seven_day":{"used_percentage":37,"resets_at":' +
        $futurLointain + '}}'
    $cacheHebdoSeul = '{"seven_day":{"used_percentage":37,"resets_at":' +
        $futurLointain + '}}'

    # Cache écrit par une version antérieure du script : plat, propre à la
    # fenêtre de 5 heures. Doit être ignoré sans bruit.
    $cacheAncienFormat = '{"used_percentage":11,"resets_at":' + $dans3h + '}'

    $socle = @{
        model     = @{ id = "claude-opus-5"; display_name = "Opus 5" }
        workspace = @{ current_dir = $projet; project_dir = $projet }
        version   = "2.1.220"
    }

    # Duplique le socle en y greffant des champs supplémentaires.
    function Avec {
        param([hashtable]$Base, [hashtable]$Ajouts)

        $copie = @{}
        foreach ($cle in $Base.Keys) { $copie[$cle] = $Base[$cle] }
        foreach ($cle in $Ajouts.Keys) { $copie[$cle] = $Ajouts[$cle] }
        return $copie
    }

    # Fabrique un cache d'une seule fenêtre, ancrage de mesure compris — et,
    # depuis le 03/09/2026, la famille de modèle sous laquelle il a été posé
    # quand le cas en fournit une. Sans elle, le cache est de la forme
    # antérieure à ce champ, ce que plusieurs cas exercent à dessein.
    function Ancre {
        param(
            [string]$Cle,
            $Pourcent,
            $Reset,
            $ObserveA,
            $ObservePourcent,
            [string]$ObserveModele
        )

        $famille = if ($ObserveModele) {
            ',"observe_modele":"' + $ObserveModele + '"'
        } else {
            ''
        }

        return '{"' + $Cle + '":{"used_percentage":' + $Pourcent +
            ',"resets_at":' + $Reset +
            ',"observe_a":' + $ObserveA +
            ',"observe_pourcent":' + $ObservePourcent + $famille + '}}'
    }

    # Payload ne portant qu'une fenêtre fraîche, sur le socle ordinaire ou sur
    # celui d'un autre modèle.
    function Fenetre {
        param([string]$Cle, $Pourcent, $Reset, [hashtable]$Socle = $socle)

        return (New-Payload (Avec $Socle @{
                    rate_limits = @{ $Cle = @{ used_percentage = $Pourcent; resets_at = $Reset } }
                }))
    }

    $cas = @()

    $cas += New-Cas "nominal complet" (New-Payload (Avec $socle @{
        context_window = @{ used_percentage = 34 }
        rate_limits    = @{ five_hour = @{ used_percentage = 29; resets_at = $dans3h } }
    }))

    $cas += New-Cas "reset un autre jour" (New-Payload (Avec $socle @{
        rate_limits = @{ five_hour = @{ used_percentage = 29; resets_at = $futurLointain } }
    }))

    $cas += New-Cas "resets_at ISO 8601" (New-Payload (Avec $socle @{
        rate_limits = @{ five_hour = @{ used_percentage = 29; resets_at = $iso } }
    }))

    $cas += New-Cas "resets_at millisecondes" (New-Payload (Avec $socle @{
        rate_limits = @{ five_hour = @{ used_percentage = 29; resets_at = ($dans3h * 1000) } }
    }))

    $cas += New-Cas "resets_at aberrant" (New-Payload (Avec $socle @{
        rate_limits = @{ five_hour = @{ used_percentage = 29; resets_at = 999999999999999 } }
    }))

    # Décalage horaire de quatre octets non ASCII — 04/09/2026 : la coupe par
    # octets du portage paniquait, et la ligne entière se réduisait au nom du
    # modèle. La date est illisible des deux côtés, et la fenêtre s'affiche
    # sans heure, remise à zéro nulle au cache.
    $cas += New-Cas "resets_at ISO non ASCII" (New-Payload (Avec $socle @{
        rate_limits = @{ five_hour = @{ used_percentage = 29; resets_at = "2026-09-05T10:00:00+aéb" } }
    }))

    # Décalage de +20:00 : aucun fuseau ne l'a, et DateTimeOffset.TryParse le
    # refuse, mais ConvertFrom-Json convertit lui-même les dates ISO du payload
    # en [datetime] et lit tout décalage à deux chiffres. L'oracle affiche
    # donc l'heure, et le portage doit la lire de même.
    $cas += New-Cas "resets_at ISO decalage hors fuseau" (New-Payload (Avec $socle @{
        rate_limits = @{ five_hour = @{ used_percentage = 29; resets_at = "2026-09-05T10:00:00+20:00" } }
    }))

    # Décalage à seize chiffres : ConvertFrom-Json le laisse en chaîne, que
    # rien ne sait lire, et le portage le refuse au lieu de multiplier par 3600
    # jusqu'au débordement.
    $cas += New-Cas "resets_at ISO decalage demesure" (New-Payload (Avec $socle @{
        rate_limits = @{ five_hour = @{ used_percentage = 29; resets_at = "2026-09-05T10:00:00+9999999999999999:00" } }
    }))

    $cas += New-Cas "valeurs non numeriques" (New-Payload (Avec $socle @{
        context_window = @{ used_percentage = "beaucoup" }
        rate_limits    = @{ five_hour = @{ used_percentage = "n/a"; resets_at = "jamais" } }
    }))

    $cas += New-Cas "pourcentages en chaine" (New-Payload (Avec $socle @{
        context_window = @{ used_percentage = "33.6" }
        rate_limits    = @{ five_hour = @{ used_percentage = "29.4"; resets_at = $dans3h } }
    }))

    $cas += New-Cas "seuils depasses sans couleur" (New-Payload (Avec $socle @{
        context_window = @{ used_percentage = 91 }
        rate_limits    = @{ five_hour = @{ used_percentage = 85; resets_at = $dans3h } }
    }))

    $cas += New-Cas "seuils depasses avec couleur" (New-Payload (Avec $socle @{
        context_window = @{ used_percentage = 91 }
        rate_limits    = @{ five_hour = @{ used_percentage = 85; resets_at = $dans3h } }
    })) -AvecCouleur

    $cas += New-Cas "juste sous les seuils avec couleur" (New-Payload (Avec $socle @{
        context_window = @{ used_percentage = 79 }
        rate_limits    = @{ five_hour = @{ used_percentage = 79; resets_at = $dans3h } }
    })) -AvecCouleur

    $cas += New-Cas "zero pour cent" (New-Payload (Avec $socle @{
        context_window = @{ used_percentage = 0 }
        rate_limits    = @{ five_hour = @{ used_percentage = 0; resets_at = $dans3h } }
    }))

    # Hors de 0 à 100 — 04/09/2026 : ramené dans les bornes des deux côtés,
    # ligne et cache. Un 150 s'affichait tel quel et projetait un épuisement
    # dans le passé ; il vaut désormais un plafond, au palier critique.
    $cas += New-Cas "pourcentage au-dela de cent" (New-Payload (Avec $socle @{
        context_window = @{ used_percentage = 150 }
        rate_limits    = @{ five_hour = @{ used_percentage = 150; resets_at = $dans3h } }
    })) -AvecCouleur

    $cas += New-Cas "pourcentage negatif" (New-Payload (Avec $socle @{
        context_window = @{ used_percentage = -5 }
        rate_limits    = @{ five_hour = @{ used_percentage = -5; resets_at = $dans3h } }
    }))

    $cas += New-Cas "sans rate_limits, cache valide" (New-Payload $socle) $cacheValide
    $cas += New-Cas "sans rate_limits, cache perime" (New-Payload $socle) $cachePerime
    $cas += New-Cas "sans rate_limits, sans cache" (New-Payload $socle)
    $cas += New-Cas "sans rate_limits, cache illisible" (New-Payload $socle) "{ceci n'est pas du json"
    $cas += New-Cas "sans rate_limits, cache incomplet" (New-Payload $socle) '{"used_percentage":11}'

    $cas += New-Cas "sans rate_limits, cache ancien format" (New-Payload $socle) `
        $cacheAncienFormat

    $cas += New-Cas "rate_limits ecrit le cache" (New-Payload (Avec $socle @{
        rate_limits = @{ five_hour = @{ used_percentage = 42; resets_at = $dans3h } }
    })) $cachePerime

    # ---- Fenêtre de 7 jours -------------------------------------------------

    $cas += New-Cas "deux fenetres" (New-Payload (Avec $socle @{
        context_window = @{ used_percentage = 34 }
        rate_limits    = @{
            five_hour = @{ used_percentage = 29; resets_at = $dans3h }
            seven_day = @{ used_percentage = 41; resets_at = $futurLointain }
        }
    }))

    $cas += New-Cas "seven_day seul" (New-Payload (Avec $socle @{
        rate_limits = @{
            seven_day = @{ used_percentage = 41; resets_at = $futurLointain }
        }
    }))

    # Sous le seuil hebdomadaire, la remise à zéro reste masquée ; au-dessus,
    # elle apparaît d'elle-même — c'est là qu'elle devient actionnable.
    $cas += New-Cas "seven_day juste sous le seuil" (New-Payload (Avec $socle @{
        rate_limits = @{
            seven_day = @{ used_percentage = 74; resets_at = $futurLointain }
        }
    }))

    $cas += New-Cas "seven_day au seuil montre le reset" (New-Payload (Avec $socle @{
        rate_limits = @{
            seven_day = @{ used_percentage = 75; resets_at = $futurLointain }
        }
    }))

    $cas += New-Cas "seven_day en alerte avec couleur" (New-Payload (Avec $socle @{
        rate_limits = @{
            seven_day = @{ used_percentage = 88; resets_at = $futurLointain }
        }
    })) -AvecCouleur

    # Les seuils diffèrent : 79 % alerte sur la semaine, pas sur les 5 heures.
    $cas += New-Cas "seuils distincts entre fenetres" (New-Payload (Avec $socle @{
        rate_limits = @{
            five_hour = @{ used_percentage = 79; resets_at = $dans3h }
            seven_day = @{ used_percentage = 79; resets_at = $futurLointain }
        }
    })) -AvecCouleur

    # Le cœur de la fusion : un payload ne portant que « five_hour » ne doit pas
    # effacer du cache la fenêtre hebdomadaire.
    $cas += New-Cas "five_hour frais preserve seven_day" (New-Payload (Avec $socle @{
        rate_limits = @{ five_hour = @{ used_percentage = 42; resets_at = $dans3h } }
    })) $cacheHebdoSeul

    $cas += New-Cas "deux fenetres depuis le cache" (New-Payload $socle) `
        $cacheDeuxFenetres

    $cas += New-Cas "seven_day null" (New-Payload (Avec $socle @{
        rate_limits = @{
            five_hour = @{ used_percentage = 29; resets_at = $dans3h }
            seven_day = $null
        }
    }))

    # Sans pourcentage lisible, la fenêtre disparaît en entier : une heure de
    # remise à zéro seule ne situe plus rien.
    $cas += New-Cas "pourcentage illisible, reset valide" (New-Payload (Avec $socle @{
        rate_limits = @{
            five_hour = @{ used_percentage = "n/a"; resets_at = $dans3h }
        }
    }))

    # ---- Effort et modes ----------------------------------------------------

    $cas += New-Cas "effort" (New-Payload (Avec $socle @{
        effort = @{ level = "xhigh" }
    }))

    $cas += New-Cas "mode rapide" (New-Payload (Avec $socle @{
        fast_mode = $true
    }))

    $cas += New-Cas "mode rapide inactif" (New-Payload (Avec $socle @{
        fast_mode = $false
    }))

    $cas += New-Cas "reflexion active" (New-Payload (Avec $socle @{
        thinking = @{ enabled = $true }
    }))

    $cas += New-Cas "reflexion coupee" (New-Payload (Avec $socle @{
        thinking = @{ enabled = $false }
    }))

    $cas += New-Cas "effort et modes cumules" (New-Payload (Avec $socle @{
        effort    = @{ level = "max" }
        fast_mode = $true
        thinking  = @{ enabled = $false }
    }))

    # Payloads dégénérés : un champ du mauvais type ne doit pas produire de
    # marqueur, « -eq » sur un tableau filtrant au lieu de comparer.
    $cas += New-Cas "effort non chaine" (New-Payload (Avec $socle @{
        effort = @{ level = 3 }
    }))

    $cas += New-Cas "effort chaine vide" (New-Payload (Avec $socle @{
        effort = @{ level = "   " }
    }))

    $cas += New-Cas "fast_mode non booleen" (New-Payload (Avec $socle @{
        fast_mode = "true"
    }))

    $cas += New-Cas "thinking non booleen" (New-Payload (Avec $socle @{
        thinking = @{ enabled = "non" }
    }))

    $cas += New-Cas "effort tableau" (New-Payload (Avec $socle @{
        effort = @{ level = @("xhigh", "max") }
    }))

    # ---- Rythme de consommation ---------------------------------------------
    #
    # L'ancrage du cache porte la première mesure vue sur la fenêtre. La pente
    # s'en déduit, puis se projette jusqu'à la remise à zéro.

    # 20 % il y a une heure, 30 % maintenant, trois heures restantes : 60 %.
    $cas += New-Cas "rythme projection" (Fenetre "five_hour" 30 $dans3h) `
    (Ancre "five_hour" 20 $dans3h $ilYa1h 20)

    # Projection à 95 % : au-delà du seuil, donc en alerte, sans être un
    # épuisement.
    $cas += New-Cas "rythme projection en alerte" (Fenetre "five_hour" 50 $dans3h) `
    (Ancre "five_hour" 35 $dans3h $ilYa1h 35) -AvecCouleur

    # 60 points en une heure : le plafond tombe vingt minutes avant la remise
    # à zéro.
    $cas += New-Cas "rythme epuisement" (Fenetre "five_hour" 80 $dans3h) `
    (Ancre "five_hour" 20 $dans3h $ilYa1h 20)

    $cas += New-Cas "rythme epuisement avec couleur" (Fenetre "five_hour" 80 $dans3h) `
    (Ancre "five_hour" 20 $dans3h $ilYa1h 20) -AvecCouleur

    $cas += New-Cas "rythme observation trop courte" (Fenetre "five_hour" 30 $dans3h) `
    (Ancre "five_hour" 25 $dans3h $ilYa5min 25)

    $cas += New-Cas "rythme rien consomme" (Fenetre "five_hour" 30 $dans3h) `
    (Ancre "five_hour" 30 $dans3h $ilYa1h 30)

    # Les trois réancrages : autre fenêtre, compteur en recul, horloge reculée.
    # Dans les trois cas la durée d'observation repart de zéro, donc pas de
    # rythme — et le cache doit porter le nouvel ancrage.
    $cas += New-Cas "rythme nouvelle fenetre" (Fenetre "five_hour" 30 $dans3h) `
    (Ancre "five_hour" 20 $dans2h $ilYa1h 20)

    $cas += New-Cas "rythme compteur en arriere" (Fenetre "five_hour" 30 $dans3h) `
    (Ancre "five_hour" 50 $dans3h $ilYa1h 50)

    $cas += New-Cas "rythme ancrage dans le futur" (Fenetre "five_hour" 30 $dans3h) `
    (Ancre "five_hour" 20 $dans3h $dans2min 20)

    # Un point en une heure, dix minutes restantes : la projection arrondie
    # rejoint le pourcentage courant et n'apprend donc rien.
    $cas += New-Cas "rythme projection sans apport" (Fenetre "five_hour" 30 $dans10min) `
    (Ancre "five_hour" 29 $dans10min $ilYa1h 29)

    # Fenêtre relue du cache : le « ~ » reste, le rythme se tait.
    $cas += New-Cas "rythme fenetre memorisee" (New-Payload $socle) `
    (Ancre "five_hour" 20 $dans3h $ilYa1h 10)

    # Cache écrit par la version qui précède le rythme : sans ancrage, donc
    # réancré à la première mesure, sans rien afficher.
    $cas += New-Cas "rythme cache sans ancrage" (Fenetre "five_hour" 30 $dans3h) `
        $cacheValide

    # Sur la semaine, l'ancrage traverse les sessions : dix points en deux
    # jours, quatre jours restants, projection à 40 %.
    $cas += New-Cas "rythme hebdomadaire" (Fenetre "seven_day" 20 $futurLointain) `
    (Ancre "seven_day" 10 $futurLointain $ilYa2j 10)

    $cas += New-Cas "rythme hebdomadaire epuisement" `
    (Fenetre "seven_day" 76 $futurLointain) `
    (Ancre "seven_day" 20 $futurLointain $ilYa2j 20) -AvecCouleur

    # ---- La fenêtre hebdomadaire suit le modèle — 03/09/2026 ---------------
    #
    # Sous Fable, deux fois plus cher qu'Opus par jeton, la semaine se vide deux
    # fois plus vite : les seuils hebdomadaires passent de 75/85 à 50/70, et
    # l'ancrage du rythme repart quand la famille du modèle change. La fenêtre
    # de 5 heures, elle, ne bouge pas. Le cache porte la famille de l'ancrage,
    # pour la seule fenêtre qui suit le modèle.
    #
    # Tous ces cas détournent la configuration vers un fichier sans relevé de
    # /usage : le compte réel du poste en porte un, qui donnerait à Fable une
    # fenêtre propre et changerait ce que ces cas mesurent — voir la série
    # suivante, qui couvre justement ce relevé.
    $socleFable = Avec $socle @{
        model = @{ id = "claude-fable-5-1"; display_name = "Fable 5.1" }
    }
    $socleSonnet = Avec $socle @{
        model = @{ id = "claude-sonnet-5"; display_name = "Sonnet 5" }
    }
    $socleSansId = Avec $socle @{
        model = @{ display_name = "Fable 5.1" }
    }
    $socleInconnu = Avec $socle @{
        model = @{ id = "claude-ultra-6"; display_name = "Ultra 6" }
    }
    $sansReleve = Join-Path $Racine "config-max.json"

    # 55 % : sous le seuil d'Opus, au-dessus de celui de Fable — l'ambre
    # s'allume, et la remise à zéro paraît avec lui.
    $cas += New-Cas "fable 7j en alerte a 55" `
    (Fenetre "seven_day" 55 $futurLointain $socleFable) -Config $sansReleve -AvecCouleur

    $cas += New-Cas "fable 7j au seuil montre le reset" `
    (Fenetre "seven_day" 50 $futurLointain $socleFable) -Config $sansReleve

    $cas += New-Cas "fable 7j juste sous le seuil a 49" `
    (Fenetre "seven_day" 49 $futurLointain $socleFable) -Config $sansReleve -AvecCouleur

    # 72 % : rien sous Opus, critique sous Fable.
    $cas += New-Cas "fable 7j critique a 72" `
    (Fenetre "seven_day" 72 $futurLointain $socleFable) -Config $sansReleve -AvecCouleur

    # La fenêtre de 5 heures garde ses seuils : 79 % n'y alerte pas davantage
    # sous Fable, alors que 49 % sur la semaine reste sous le seuil abaissé.
    $cas += New-Cas "fable 5h garde ses seuils" (New-Payload (Avec $socleFable @{
        rate_limits = @{
            five_hour = @{ used_percentage = 79; resets_at = $dans3h }
            seven_day = @{ used_percentage = 49; resets_at = $futurLointain }
        }
    })) -Config $sansReleve -AvecCouleur

    # Un modèle moins cher ne relève rien : 76 % reste en alerte sous Sonnet.
    $cas += New-Cas "sonnet 7j garde le seuil d'opus" `
    (Fenetre "seven_day" 76 $futurLointain $socleSonnet) -Config $sansReleve -AvecCouleur

    # La famille se lit aussi dans le nom affiché, faute d'identifiant.
    $cas += New-Cas "fable sans id, nom affiche" `
    (Fenetre "seven_day" 55 $futurLointain $socleSansId) -Config $sansReleve -AvecCouleur

    # Un modèle hors table garde les seuils d'Opus.
    $cas += New-Cas "modele hors table, seuils inchanges" `
    (Fenetre "seven_day" 55 $futurLointain $socleInconnu) -Config $sansReleve -AvecCouleur

    # Fenêtre relue du cache : le « ~55% » se colore aux seuils de la session
    # qui va consommer, et la famille mémorisée est réécrite telle quelle.
    $cas += New-Cas "fable memorisee coloree aux seuils du jour" (New-Payload $socleFable) `
    (Ancre "seven_day" 55 $futurLointain $ilYa2j 40 "opus") -Config $sansReleve -AvecCouleur

    # Quinze points en deux jours, quatre jours restants : projection à 65 %,
    # sous le seuil d'Opus, au-dessus de celui de Fable — donc en ambre.
    $cas += New-Cas "fable projection coloree au seuil abaisse" `
    (Fenetre "seven_day" 35 $futurLointain $socleFable) `
    (Ancre "seven_day" 30 $futurLointain $ilYa2j 20 "fable") -Config $sansReleve -AvecCouleur

    # L'ancrage a été posé sous Opus, la session tourne sous Fable : la pente
    # d'Opus ne dit rien de Fable, l'ancrage repart et le rythme se tait. Le
    # cache doit porter le nouvel ancrage, étiqueté « fable ».
    $cas += New-Cas "rythme reancre au changement de famille" `
    (Fenetre "seven_day" 20 $futurLointain $socleFable) `
    (Ancre "seven_day" 10 $futurLointain $ilYa2j 10 "opus") -Config $sansReleve

    # Même famille : l'ancrage tient, et le rythme s'affiche.
    $cas += New-Cas "rythme conserve dans la meme famille" `
    (Fenetre "seven_day" 20 $futurLointain $socleFable) `
    (Ancre "seven_day" 10 $futurLointain $ilYa2j 10 "fable") -Config $sansReleve

    # Cache antérieur au champ : l'ancrage tient, et prend la famille du jour.
    $cas += New-Cas "rythme cache sans famille conserve l'ancrage" `
    (Fenetre "seven_day" 20 $futurLointain $socleFable) `
    (Ancre "seven_day" 10 $futurLointain $ilYa2j 10) -Config $sansReleve

    # Modèle hors table : la famille est inconnue, rien ne réancre, et le cache
    # garde la famille mémorisée.
    $cas += New-Cas "rythme modele inconnu conserve l'ancrage" `
    (Fenetre "seven_day" 20 $futurLointain $socleInconnu) `
    (Ancre "seven_day" 10 $futurLointain $ilYa2j 10 "opus") -Config $sansReleve

    # La fenêtre de 5 heures ignore la famille : l'ancrage posé sous Opus sert
    # tel quel sous Fable, et le champ est réécrit à null.
    $cas += New-Cas "rythme 5h ignore la famille" `
    (Fenetre "five_hour" 30 $dans3h $socleFable) `
    (Ancre "five_hour" 20 $dans3h $ilYa1h 20 "opus") -Config $sansReleve

    # ---- La fenêtre du modèle, relevée par /usage — 03/09/2026 au soir -----
    #
    # /usage affiche pour Fable une fenêtre hebdomadaire propre, que Claude Code
    # persiste dans ~/.claude.json et que le payload ne porte pas. La ligne la
    # lit, la mesure sous sa propre clé de cache « seven_day_modele », à
    # l'instant de son relevé, et lui donne la place « 7j » quand elle est plus
    # remplie que la fenêtre globale. Les fichiers sont fabriqués par
    # Initialize-Bac, sur le même instant que ces cas.
    $releveFable = Join-Path $Racine "config-usage-fable.json"

    # Le relevé du poste : 14 % pour tous, 28 % pour Fable. La ligne dit 28,
    # sous les seuils ordinaires — ce budget est déjà celui de Fable.
    $cas += New-Cas "usage fable prend la fenetre du modele" `
    (Fenetre "seven_day" 14 $futurLointain $socleFable) -Config $releveFable

    $cas += New-Cas "usage fable avec couleur" `
    (Fenetre "seven_day" 14 $futurLointain $socleFable) -Config $releveFable -AvecCouleur

    # Fenêtre globale en alerte — 60 %, au-dessus du seuil abaissé de Fable —
    # et plus remplie que celle du modèle : c'est elle qui menace la session,
    # elle garde la place, et ses seuils abaissés avec. Le relevé porte la même
    # globale que le payload : sans quoi la dérive du 05/09/2026 avancerait la
    # fenêtre du modèle, et le cas ne mesurerait plus la règle d'attribution.
    $cas += New-Cas "usage fable globale en alerte l'emporte" `
    (Fenetre "seven_day" 60 $futurLointain $socleFable) `
    -Config (Join-Path $Racine "config-usage-fable-globale-60.json") -AvecCouleur

    # Fenêtre globale plus remplie mais sans alerte — 30 % contre 28 % : la
    # fenêtre du modèle garde la place, c'est elle qui décrit la session.
    # Règle du 04/09/2026 ; la précédente aurait affiché 30. Même relevé à la
    # globale du payload, pour la même raison.
    $cas += New-Cas "usage fable globale plus remplie sans alerte" `
    (Fenetre "seven_day" 30 $futurLointain $socleFable) `
    -Config (Join-Path $Racine "config-usage-fable-globale-30.json")

    # ---- Fenêtre pas encore entamée — 04/09/2026 ---------------------------
    #
    # Le lendemain d'une remise à zéro hebdomadaire, le relevé porte pour Fable
    # 0 % et une remise à zéro nulle. /usage affiche la barre sans sa ligne
    # « Resets » ; la ligne affiche « 7j ░░ 0% », sans heure ni rythme, et
    # mémorise l'échéance nulle. La globale à 2 % ne l'emporte pas : elle n'a
    # rien à signaler.
    $releveVierge = Join-Path $Racine "config-usage-fable-vierge.json"

    $cas += New-Cas "usage fable vierge sans echeance" `
    (Fenetre "seven_day" 2 $futurLointain $socleFable) -Config $releveVierge

    $cas += New-Cas "usage fable vierge avec couleur" `
    (Fenetre "seven_day" 2 $futurLointain $socleFable) -Config $releveVierge -AvecCouleur

    # Même relevé, globale en alerte à 55 % : elle reprend la place.
    $cas += New-Cas "usage fable vierge, globale en alerte" `
    (Fenetre "seven_day" 55 $futurLointain $socleFable) -Config $releveVierge -AvecCouleur

    # Avant la première réponse de la session, sans cache : la fenêtre vierge
    # suffit à afficher la semaine.
    $cas += New-Cas "usage fable vierge sans rate_limits" (New-Payload $socleFable) `
    -Config $releveVierge

    # Fenêtre du modèle à 80 % : alerte au seuil ordinaire, remise à zéro
    # affichée.
    $cas += New-Cas "usage fable fenetre du modele en alerte" `
    (Fenetre "seven_day" 14 $futurLointain $socleFable) `
    -Config (Join-Path $Racine "config-usage-fable-alerte.json") -AvecCouleur

    # Relevé vieux de deux heures : la valeur reste, marquée « ~ », sans rythme.
    $cas += New-Cas "usage fable releve ancien" `
    (Fenetre "seven_day" 14 $futurLointain $socleFable) `
    -Config (Join-Path $Racine "config-usage-fable-ancien.json")

    # Quatre relevés qui ne disent rien d'utilisable : la fenêtre globale reste
    # seule, comme sans relevé.
    $cas += New-Cas "usage fable fenetre expiree" `
    (Fenetre "seven_day" 14 $futurLointain $socleFable) `
    -Config (Join-Path $Racine "config-usage-fable-expiree.json")

    $cas += New-Cas "usage d'un autre modele ignore" `
    (Fenetre "seven_day" 14 $futurLointain $socleFable) `
    -Config (Join-Path $Racine "config-usage-sonnet.json")

    $cas += New-Cas "usage invalide ignore" `
    (Fenetre "seven_day" 14 $futurLointain $socleFable) `
    -Config (Join-Path $Racine "config-usage-invalide.json")

    $cas += New-Cas "usage d'un autre compte ignore" `
    (Fenetre "seven_day" 14 $futurLointain $socleFable) `
    -Config (Join-Path $Racine "config-usage-autre-compte.json")

    # Sous Opus, la fenêtre Fable du relevé ne concerne pas la session.
    $cas += New-Cas "usage sous opus ignore la fenetre fable" `
    (Fenetre "seven_day" 14 $futurLointain) -Config $releveFable

    # Avant la première réponse de la session, sans cache : le relevé suffit à
    # afficher la semaine.
    $cas += New-Cas "usage fable sans rate_limits" (New-Payload $socleFable) `
    -Config $releveFable

    # Dix-huit points en deux jours sur la fenêtre du modèle, quatre jours
    # restants : projection à 64 %. L'ancrage vit sous sa propre clé.
    $cas += New-Cas "usage fable rythme sur la fenetre du modele" `
    (Fenetre "seven_day" 14 $futurLointain $socleFable) `
    (Ancre "seven_day_modele" 20 $futurLointain $ilYa2j 10 "fable") -Config $releveFable

    # Ancrage posé sous une autre famille : il repart, étiqueté « fable ».
    $cas += New-Cas "usage fable reancre au changement de famille" `
    (Fenetre "seven_day" 14 $futurLointain $socleFable) `
    (Ancre "seven_day_modele" 20 $futurLointain $ilYa2j 10 "sonnet") -Config $releveFable

    # Les trois fenêtres au cache, dans l'ordre : 5 heures, 7 jours, modèle.
    $cas += New-Cas "usage fable ecrit trois fenetres" (New-Payload (Avec $socleFable @{
        rate_limits = @{
            five_hour = @{ used_percentage = 29; resets_at = $dans3h }
            seven_day = @{ used_percentage = 14; resets_at = $futurLointain }
        }
    })) -Config $releveFable

    # ---- La dérive entre deux relevés — 05/09/2026 --------------------------
    #
    # Le relevé ne bouge qu'à l'ouverture de /usage, mais la fenêtre globale du
    # payload est fraîche à chaque réponse : ce qu'elle a pris depuis le relevé,
    # la fenêtre du modèle l'a pris aussi, au rapport des deux budgets — 28
    # contre 14 dans le relevé, soit deux. La valeur estimée s'affiche marquée
    # « ≈ », sans rythme ; le cache garde le relevé.
    $releveAncien = Join-Path $Racine "config-usage-fable-ancien.json"

    # Deux points pris par la globale : 28 + 2 × 2 = 32.
    $cas += New-Cas "derive fable globale avancee de deux points" `
    (Fenetre "seven_day" 16 $futurLointain $socleFable) -Config $releveFable

    $cas += New-Cas "derive fable avec couleur" `
    (Fenetre "seven_day" 16 $futurLointain $socleFable) -Config $releveFable -AvecCouleur

    # Relevé vieux de deux heures : l'estimation l'emporte sur le « ~ ».
    $cas += New-Cas "derive fable releve ancien" `
    (Fenetre "seven_day" 16 $futurLointain $socleFable) -Config $releveAncien

    # Vingt-six points : 28 + 52 = 80, au seuil ordinaire — ambre, et la remise
    # à zéro paraît, sur la valeur estimée.
    $cas += New-Cas "derive fable jusqu'a l'alerte" `
    (Fenetre "seven_day" 40 $futurLointain $socleFable) -Config $releveFable -AvecCouleur

    # Quarante-six points : 120, plafonné à 100 — critique. La globale à 60 %
    # alerte aussi, mais n'est pas la plus remplie : la fenêtre du modèle garde
    # la place.
    $cas += New-Cas "derive fable plafonnee a cent" `
    (Fenetre "seven_day" 60 $futurLointain $socleFable) -Config $releveFable -AvecCouleur

    # Globale en recul depuis le relevé : ce n'est pas une consommation, le
    # relevé s'affiche tel quel, sans marqueur.
    $cas += New-Cas "derive fable globale en recul" `
    (Fenetre "seven_day" 10 $futurLointain $socleFable) -Config $releveFable

    # Globale du payload sur une autre remise à zéro : ce n'est plus la même
    # fenêtre, rien à comparer.
    $cas += New-Cas "derive fable autre fenetre globale" `
    (Fenetre "seven_day" 16 $dans3h $socleFable) -Config $releveFable

    # Relevés sans point de référence : pas d'entrée globale, ou une globale
    # sans échéance.
    $cas += New-Cas "derive fable releve sans globale" `
    (Fenetre "seven_day" 16 $futurLointain $socleFable) `
    -Config (Join-Path $Racine "config-usage-fable-sans-globale.json")

    $cas += New-Cas "derive fable globale sans echeance" `
    (Fenetre "seven_day" 16 $futurLointain $socleFable) `
    -Config (Join-Path $Racine "config-usage-fable-globale-sans-echeance.json")

    # Fenêtre vierge — 0 % contre 2 % : le rapport n'existe pas, et la ligne
    # dit 0 tant que /usage n'a pas été rouvert.
    $cas += New-Cas "derive fable vierge sans rapport" `
    (Fenetre "seven_day" 10 $futurLointain $socleFable) -Config $releveVierge

    # Ancrage vivant sous la clé du modèle : le rythme se serait affiché sur le
    # relevé, il se tait sur l'estimation, et le cache garde le relevé à 28.
    $cas += New-Cas "derive fable rythme tu" `
    (Fenetre "seven_day" 16 $futurLointain $socleFable) `
    (Ancre "seven_day_modele" 20 $futurLointain $ilYa2j 10 "fable") -Config $releveFable

    # Non-régression : le rythme se taisait dès lors qu'il avait observé dix
    # minutes, quelle que soit la durée sur laquelle il extrapolait ensuite.
    # Un point gagné en une heure, projeté sur les quatre jours d'une fenêtre
    # hebdomadaire, annonçait un épuisement pour le lendemain à 5 % de
    # consommation — constaté en usage réel. Le segment doit désormais se
    # retirer entièrement.
    $cas += New-Cas "rythme extrapolation excessive" `
    (Fenetre "seven_day" 5 $futurLointain) `
    (Ancre "seven_day" 4 $futurLointain $ilYa1h 4)

    # Les deux cas qui encadrent la borne ne diffèrent que par l'éloignement de
    # la remise à zéro : à ancrage et consommation identiques, seul le rapport
    # d'extrapolation décide. Sous la borne, la projection s'affiche à 55 % ;
    # au-dessus, il ne reste que le pourcentage courant.
    $cas += New-Cas "rythme extrapolation sous la borne" `
    (Fenetre "five_hour" 30 $dans5h) `
    (Ancre "five_hour" 25 $dans5h $ilYa1h 25)

    $cas += New-Cas "rythme extrapolation au-dela de la borne" `
    (Fenetre "five_hour" 30 $dans7h) `
    (Ancre "five_hour" 25 $dans7h $ilYa1h 25)

    $cas += New-Cas "five_hour null" (New-Payload (Avec $socle @{
        rate_limits = @{ five_hour = $null }
    }))

    $cas += New-Cas "resets_at absent" (New-Payload (Avec $socle @{
        rate_limits = @{ five_hour = @{ used_percentage = 29 } }
    }))

    $cas += New-Cas "context_window null" (New-Payload (Avec $socle @{
        context_window = $null
    }))

    $cas += New-Cas "modele absent" (New-Payload @{
        workspace = @{ current_dir = $projet; project_dir = $projet }
    })

    $cas += New-Cas "workspace absent, cwd seul" (New-Payload @{
        model = @{ display_name = "Opus 5" }
        cwd   = $projet
    })

    $cas += New-Cas "aucun emplacement" (New-Payload @{
        model = @{ display_name = "Opus 5" }
    })

    $cas += New-Cas "sous-dossier a un niveau" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{
            current_dir = (Join-Path $projet "src")
            project_dir = $projet
        }
    })

    $cas += New-Cas "sous-dossier profond, repli" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{
            current_dir = (Join-Path $projet "src\api\v2\interne")
            project_dir = $projet
        }
    })

    $cas += New-Cas "sous-dossier hors projet" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{
            current_dir = (Join-Path $Racine "ailleurs")
            project_dir = $projet
        }
    })

    $cas += New-Cas "casse differente du projet" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{
            current_dir = (Join-Path $projet "SRC")
            project_dir = $projet.ToUpper()
        }
    })

    $cas += New-Cas "branche lue dans un sous-dossier" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{
            current_dir = (Join-Path $projet "src\api")
            project_dir = $projet
        }
    })

    $cas += New-Cas "HEAD detachee" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{
            current_dir = (Join-Path $Racine "detache")
            project_dir = (Join-Path $Racine "detache")
        }
    })

    $cas += New-Cas "git fichier gitdir" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{
            current_dir = (Join-Path $Racine "pointeur")
            project_dir = (Join-Path $Racine "pointeur")
        }
    })

    $cas += New-Cas "worktree.branch prioritaire" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{ current_dir = $projet; project_dir = $projet }
        worktree  = @{ branch = "release/2026" }
    })

    # Casse à longueur d'octets variable — 04/09/2026 : « Ⱥ » fait deux octets
    # en UTF-8 là où « ⱥ » en fait trois, et OrdinalIgnoreCase les fait se
    # valoir. Le portage coupait l'original à la longueur du préfixe mis en
    # minuscules, un octet trop loin ; l'oracle, en UTF-16, n'a jamais eu le
    # problème. Attendu des deux côtés : « ⱥrbre\src ».
    $cas += New-Cas "casse a longueur variable" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{
            current_dir = "C:\aucun-dossier-4c1f\" + [char]0x023A + "rbre\src"
            project_dir = "C:\aucun-dossier-4c1f\" + [char]0x2C65 + "rbre"
        }
    })

    # Le signe Kelvin « K » n'est pas un « k » pour OrdinalIgnoreCase, qui
    # écarte les correspondances non réversibles : un autre chemin, la feuille
    # seule.
    $cas += New-Cas "casse kelvin hors projet" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{
            current_dir = "C:\aucun-dossier-4c1f\" + [char]0x212A + "elvin\src"
            project_dir = "C:\aucun-dossier-4c1f\kelvin"
        }
    })

    $cas += New-Cas "chemin accentue" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{
            current_dir = (Join-Path $Racine "données\été")
            project_dir = (Join-Path $Racine "données")
        }
    })

    # Chemins de repli du segment de version. Le binaire est détourné vers un
    # fichier absent ou dépourvu de métadonnées, ce que le poste réel ne permet
    # pas de reproduire.
    $absent = Join-Path $Racine "aucun-binaire.exe"
    $sansVersion = Join-Path $Racine "sans-version.exe"

    $cas += New-Cas "binaire introuvable, repli payload" (New-Payload $socle) `
        -Binaire $absent

    $cas += New-Cas "binaire sans metadonnees, repli payload" (New-Payload $socle) `
        -Binaire $sansVersion

    $cas += New-Cas "binaire introuvable, version absente" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{ current_dir = $projet; project_dir = $projet }
    }) -Binaire $absent

    $cas += New-Cas "binaire introuvable, version non chaine" (New-Payload @{
        model     = @{ display_name = "Opus 5" }
        workspace = @{ current_dir = $projet; project_dir = $projet }
        version   = 2226
    }) -Binaire $absent

    # Segment d'abonnement. Le compte réel du poste n'en couvre qu'une forme, et
    # elle change avec l'abonnement de l'utilisateur : la configuration est donc
    # détournée vers des fichiers maîtrisés, comme le binaire juste au-dessus.
    $cas += New-Cas "abonnement max avec palier" (New-Payload $socle) `
        -Config (Join-Path $Racine "config-max.json")

    # Un type hors table reste lisible : c'est ce qui fait qu'un abonnement
    # apparu après cette version s'affiche sans qu'on ait rien à recompiler.
    $cas += New-Cas "abonnement type inconnu" (New-Payload $socle) `
        -Config (Join-Path $Racine "config-inconnu.json")

    # Les trois chemins de repli, où le segment se retire sans emporter le reste
    # de la ligne : compte absent, JSON tronqué — le fichier est réécrit par
    # Claude Code en cours de session — et fichier introuvable.
    $cas += New-Cas "abonnement sans compte" (New-Payload $socle) `
        -Config (Join-Path $Racine "config-sans-compte.json")

    $cas += New-Cas "abonnement json tronque" (New-Payload $socle) `
        -Config (Join-Path $Racine "config-tronquee.json")

    $cas += New-Cas "abonnement config introuvable" (New-Payload $socle) `
        -Config (Join-Path $Racine "aucune-config.json")

    # Détournement et coloration ensemble : l'abonnement doit sortir en chrome
    # atténué, comme la version qui le suit, et non dans la couleur du thème.
    $cas += New-Cas "abonnement max avec couleur" (New-Payload $socle) `
        -Config (Join-Path $Racine "config-max.json") -AvecCouleur

    $cas += New-Cas "json invalide" "{ceci n'est pas du json"
    $cas += New-Cas "payload vide" ""
    $cas += New-Cas "payload blanc" "   "
    $cas += New-Cas "json nul" "null"
    $cas += New-Cas "json tableau" "[1,2,3]"

    # Nom de modèle blanc, tous les autres segments hors service : la ligne
    # assemblée ne porte plus qu'un espace, et le repli du repli doit prendre
    # le relais. C'est le seul payload bien formé qui atteint le dernier étage
    # de Confirm-Ligne.
    $cas += New-Cas "modele blanc, ligne a sauver" (New-Payload @{
        model = @{ display_name = "   " }
    }) -Binaire $absent

    return $cas
}

# --------------------------------------------------------------------------
# Exécution
# --------------------------------------------------------------------------

# Exécute un cas et rend ce qu'un appelant peut observer : la ligne écrite et
# le cache laissé derrière.
function Invoke-Cas {
    param(
        [string]$Script,
        $Cas,
        [string]$Racine
    )

    $etat = Join-Path $Racine "claude-code"
    if (Test-Path -LiteralPath $etat) {
        Remove-Item -LiteralPath $etat -Recurse -Force
    }

    if ($Cas.Cache) {
        New-Item -ItemType Directory -Path $etat -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $etat "statusline-cache.json") `
            -Value $Cas.Cache -Encoding UTF8 -NoNewline
    }

    # L'environnement est posé ici, et hérité par le processus fils.
    $env:LOCALAPPDATA = $Racine
    if ($Cas.Couleur) {
        Remove-Item Env:\NO_COLOR -ErrorAction SilentlyContinue
    } else {
        $env:NO_COLOR = "1"
    }
    if ($Cas.Binaire) {
        $env:CLAUDE_STATUSLINE_BINAIRE = $Cas.Binaire
    } else {
        Remove-Item Env:\CLAUDE_STATUSLINE_BINAIRE -ErrorAction SilentlyContinue
    }
    if ($Cas.Config) {
        $env:CLAUDE_STATUSLINE_CONFIG = $Cas.Config
    } else {
        Remove-Item Env:\CLAUDE_STATUSLINE_CONFIG -ErrorAction SilentlyContinue
    }

    # Un candidat « .exe » est lancé directement : c'est ce qui permet de
    # comparer le portage natif au script d'origine sur les mêmes cas, le
    # harnais devenant alors l'oracle de la migration.
    $sortie = if ($Script -like "*.exe") {
        ($Cas.Payload | & $Script 2>&1) -join "`n"
    } else {
        ($Cas.Payload |
            pwsh -NoProfile -ExecutionPolicy Bypass -File $Script 2>&1) -join "`n"
    }
    $code = $LASTEXITCODE

    $fichier = Join-Path $etat "statusline-cache.json"
    $cache = if (Test-Path -LiteralPath $fichier) {
        (Get-Content -LiteralPath $fichier -Raw -Encoding UTF8)
    } else {
        "(absent)"
    }

    # Les résidus « .tmp » signalent une écriture de cache interrompue.
    $residus = if (Test-Path -LiteralPath $etat) {
        @(Get-ChildItem -LiteralPath $etat -Filter "*.tmp" -ErrorAction SilentlyContinue).Count
    } else {
        0
    }

    return [pscustomobject]@{
        Ligne   = $sortie.TrimEnd("`r", "`n")
        Cache   = $cache
        Residus = $residus
        Code    = $code
    }
}

# Neutralise dans un cache lu l'ancrage qui vient d'être posé.
#
# « observe_a » porte l'instant de l'écriture. Le mode comparaison lance les
# deux scripts l'un après l'autre, à une seconde d'intervalle environ, et un
# ancrage posé à ce moment-là diverge dès que les deux lancements tombent de
# part et d'autre d'une seconde entière — soit une fois sur deux. Un script
# comparé à lui-même sortait ainsi quinze divergences sur quatre-vingt-un cas,
# toutes fausses, ce qui rendait le mode inutilisable pour ce qu'il vérifie.
#
# Seul un ancrage récent est neutralisé. Ceux que les cas injectent dans le
# cache sont datés de plusieurs minutes au moins, et restent comparés tels
# quels : ce sont eux qui portent la garantie « un ancrage existant n'est pas
# réécrit ».
function Clear-AncrageFrais {
    param([string]$Cache)

    $maintenant = [System.DateTimeOffset]::UtcNow.ToUnixTimeSeconds()

    return [regex]::Replace($Cache, '"observe_a":(\d+)', {
            param($correspondance)

            $valeur = [long]$correspondance.Groups[1].Value
            if ([Math]::Abs($maintenant - $valeur) -le 10) {
                return '"observe_a":<frais>'
            }

            return $correspondance.Value
        })
}

# Rend une ligne lisible dans un terminal : les échappements ANSI sont montrés.
function Format-Lisible {
    param([string]$Texte)

    if ([string]::IsNullOrEmpty($Texte)) {
        return "(vide)"
    }

    return ($Texte -replace [char]27, "<ESC>")
}

# --------------------------------------------------------------------------
# Programme principal
# --------------------------------------------------------------------------

if (-not (Test-Path -LiteralPath $Candidat)) {
    throw "Script candidat introuvable : $Candidat"
}
if ($Reference -and -not (Test-Path -LiteralPath $Reference)) {
    throw "Script de référence introuvable : $Reference"
}

$racine = Join-Path ([System.IO.Path]::GetTempPath()) "statusline-bac-$PID"
$localAppDataInitial = $env:LOCALAPPDATA
$noColorInitial = $env:NO_COLOR
$binaireInitial = $env:CLAUDE_STATUSLINE_BINAIRE
$configInitiale = $env:CLAUDE_STATUSLINE_CONFIG

try {
    # Un seul instant pour le bac et les cas : les relevés de /usage fabriqués
    # par l'un doivent porter les mêmes remises à zéro que les caches injectés
    # par l'autre, sans quoi aucun ancrage ne tiendrait sur la fenêtre du
    # modèle.
    $instant = [System.DateTimeOffset]::UtcNow
    Initialize-Bac -Racine $racine -Instant $instant
    $cas = Get-Cas -Racine $racine -Instant $instant

    $divergences = 0
    $ignores = 0

    foreach ($c in $cas) {
        # Cas coloré sauté avant toute exécution : ni le candidat ni la
        # référence ne sont lancés, le cache du bac n'en garde donc rien.
        if ($Reference -and $IgnorerCouleur -and $c.Couleur) {
            $ignores++
            "  --   {0}  (couleur, non compare)" -f $c.Nom
            continue
        }

        $obtenu = Invoke-Cas -Script $Candidat -Cas $c -Racine $racine

        if (-not $Reference) {
            "{0,-36} -> {1}" -f $c.Nom, (Format-Lisible $obtenu.Ligne)
            if ($obtenu.Residus -gt 0) {
                "{0,-36}    residus .tmp : {1}" -f "", $obtenu.Residus
            }
            if ($obtenu.Code -ne 0) {
                "{0,-36}    CODE DE SORTIE : {1}" -f "", $obtenu.Code
            }
            continue
        }

        $attendu = Invoke-Cas -Script $Reference -Cas $c -Racine $racine

        $cacheObtenu = Clear-AncrageFrais $obtenu.Cache
        $cacheAttendu = Clear-AncrageFrais $attendu.Cache

        $ecarts = @()
        if ($obtenu.Ligne -cne $attendu.Ligne) { $ecarts += "ligne" }
        if ($cacheObtenu -cne $cacheAttendu) { $ecarts += "cache" }
        if ($obtenu.Residus -ne $attendu.Residus) { $ecarts += "residus" }
        if ($obtenu.Code -ne $attendu.Code) { $ecarts += "code" }

        if ($ecarts.Count -eq 0) {
            "  ok   {0}" -f $c.Nom
            continue
        }

        $divergences++
        "  ECART {0}  [{1}]" -f $c.Nom, ($ecarts -join ", ")
        if ($ecarts -contains "ligne") {
            "         reference : {0}" -f (Format-Lisible $attendu.Ligne)
            "         candidat  : {0}" -f (Format-Lisible $obtenu.Ligne)
        }
        if ($ecarts -contains "cache") {
            "         cache reference : {0}" -f $cacheAttendu
            "         cache candidat  : {0}" -f $cacheObtenu
        }
        if ($ecarts -contains "code") {
            "         code reference : {0}" -f $attendu.Code
            "         code candidat  : {0}" -f $obtenu.Code
        }
    }

    ""
    if (-not $Reference) {
        "{0} cas executes." -f $cas.Count
        exit 0
    }

    $compares = $cas.Count - $ignores
    if ($ignores -gt 0) {
        "{0} cas colores non compares : voir les tests du crate (cargo test)." -f $ignores
    }

    if ($divergences -eq 0) {
        "Aucune divergence sur {0} cas." -f $compares
        exit 0
    }

    "{0} divergence(s) sur {1} cas." -f $divergences, $compares
    exit 1
}
finally {
    $env:LOCALAPPDATA = $localAppDataInitial
    if ($null -eq $noColorInitial) {
        Remove-Item Env:\NO_COLOR -ErrorAction SilentlyContinue
    } else {
        $env:NO_COLOR = $noColorInitial
    }
    if ($null -eq $binaireInitial) {
        Remove-Item Env:\CLAUDE_STATUSLINE_BINAIRE -ErrorAction SilentlyContinue
    } else {
        $env:CLAUDE_STATUSLINE_BINAIRE = $binaireInitial
    }
    if ($null -eq $configInitiale) {
        Remove-Item Env:\CLAUDE_STATUSLINE_CONFIG -ErrorAction SilentlyContinue
    } else {
        $env:CLAUDE_STATUSLINE_CONFIG = $configInitiale
    }

    if (Test-Path -LiteralPath $racine) {
        Remove-Item -LiteralPath $racine -Recurse -Force -ErrorAction SilentlyContinue
    }
}
