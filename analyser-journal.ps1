<#
.SYNOPSIS
    Active, arrete et depouille le journal de diagnostic de la ligne de statut.

.DESCRIPTION
    Repond a une seule question : quand la ligne de statut clignote, est-ce
    parce que le programme s'est tu, ou sans qu'il y soit pour rien ?

    Claude Code pousse le resultat de chaque lancement tel quel, y compris
    indefini (fonction `sz0` du binaire : `onResult(l)` est appele avant meme
    que `l` soit teste). Trois causes produisent cet indefini, et le programme
    n'en controle qu'une : la sienne, un lancement qui echoue avant lui
    (`spawn_failed`), un code de sortie non nul. Vues de l'interface, les trois
    se ressemblent : la ligne s'efface jusqu'au rafraichissement suivant.

    D'ou la lecture en creux : ce qui MANQUE au journal designe ce qui n'a
    jamais demarre. Un intervalle plus long que le `refreshInterval` est un
    lancement perdu, donc une ligne effacee ; une cadence reguliere sans trou
    disculpe au contraire le programme, et renvoie le clignotement au redessin
    de l'interface par le terminal.

    Le journal est ecrit par `statusline.exe` lui-meme, uniquement tant que son
    fichier temoin existe. Les deux fichiers vivent hors du projet, sous
    %LOCALAPPDATA%\claude-code : ils portent le rythme de travail de la
    session, et le workspace est sur un lecteur Google Drive.

.PARAMETER Activer
    Pose le fichier temoin. Prend effet au rafraichissement suivant, sans
    redemarrer la session : c'est tout l'interet du temoin sur une variable
    d'environnement, qui ne se lirait qu'au lancement de Claude Code.

.PARAMETER Arreter
    Retire le fichier temoin. Le journal cesse de grossir, et reste lisible.

.PARAMETER Effacer
    Vide le journal avant de commencer une nouvelle observation.

.PARAMETER SeuilTrouMs
    Intervalle a partir duquel deux lancements consecutifs sont juges trop
    espaces. Defaut 2000 ms, soit le double du `refreshInterval` d'une seconde.

.PARAMETER Detail
    Affiche chaque trou avec son heure, au lieu des dix premiers.

.PARAMETER Autour
    Heure d'un clignotement observe a l'oeil, au format HH:mm:ss. Affiche les
    lancements de la fenetre encadrante, un par ligne. C'est la mesure la plus
    decisive du lot : si le programme a bien tourne a cette seconde-la, et rendu
    la meme ligne qu'avant et apres, le clignotement ne lui appartient pas.

.PARAMETER FenetreS
    Demi-largeur de la fenetre de -Autour, en secondes. Defaut 3.

.EXAMPLE
    # Observer une minute de travail, puis depouiller.
    .\analyser-journal.ps1 -Activer -Effacer
    # ... laisser Claude Code travailler ...
    .\analyser-journal.ps1 -Arreter
    .\analyser-journal.ps1
#>
[CmdletBinding()]
param(
    [switch]$Activer,
    [switch]$Arreter,
    [switch]$Effacer,
    [int]$SeuilTrouMs = 2000,
    [switch]$Detail,
    [string]$Autour,
    [int]$FenetreS = 3
)

$ErrorActionPreference = 'Stop'

$racine = if ($env:LOCALAPPDATA) { $env:LOCALAPPDATA } else { $env:TEMP }
$dossier = Join-Path $racine "claude-code"
$journal = Join-Path $dossier "statusline-journal.jsonl"
$temoin = Join-Path $dossier "statusline-journal.on"

if ($Activer) {
    if (-not (Test-Path $dossier)) { New-Item -ItemType Directory -Path $dossier -Force | Out-Null }
    if (-not (Test-Path $temoin)) { New-Item -ItemType File -Path $temoin | Out-Null }
    Write-Output "Journal actif : $journal"
}

if ($Effacer -and (Test-Path $journal)) {
    Remove-Item -Path $journal -Force
    Write-Output "Journal vide."
}

if ($Arreter) {
    if (Test-Path $temoin) { Remove-Item -Path $temoin -Force }
    Write-Output "Journal arrete."
}

if ($Activer -or $Arreter -or $Effacer) {
    if (-not $Activer) { Write-Output "" } else { return }
}

if (-not (Test-Path $journal)) {
    Write-Output "Aucun journal a depouiller. Lancer d'abord : .\analyser-journal.ps1 -Activer"
    return
}

# --- Lecture -------------------------------------------------------------
# Une ligne JSON par lancement. Une ligne tronquee est ignoree plutot que
# fatale : deux lancements peuvent se chevaucher quand Claude Code relance la
# ligne avant d'avoir tue le tir precedent.
$tirs = [System.Collections.Generic.List[object]]::new()
foreach ($ligne in [System.IO.File]::ReadLines($journal)) {
    if ([string]::IsNullOrWhiteSpace($ligne)) { continue }
    try { $tirs.Add(($ligne | ConvertFrom-Json)) } catch { continue }
}

if ($tirs.Count -lt 2) {
    Write-Output "Journal trop court ($($tirs.Count) lancement(s)) : laisser tourner plus longtemps."
    return
}

$tirs = $tirs | Sort-Object t
$debut = [DateTimeOffset]::FromUnixTimeMilliseconds($tirs[0].t).ToLocalTime()
$fin = [DateTimeOffset]::FromUnixTimeMilliseconds($tirs[-1].t).ToLocalTime()
$duree = ($tirs[-1].t - $tirs[0].t) / 1000.0

Write-Output "=== Journal de la ligne de statut ==="
Write-Output "Periode      : $($debut.ToString('HH:mm:ss')) -> $($fin.ToString('HH:mm:ss'))  ($([math]::Round($duree,1)) s)"
Write-Output "Lancements   : $($tirs.Count)  (soit un toutes les $([math]::Round($duree / [math]::Max(1, $tirs.Count - 1), 2)) s en moyenne)"

# --- Voies empruntees ----------------------------------------------------
# « ok » est l'assemblage complet ; toute autre voie est un repli, donc une
# ligne degradee que l'oeil peut prendre pour une disparition.
$parVoie = $tirs | Group-Object voie | Sort-Object Count -Descending
$resumeVoies = ($parVoie | ForEach-Object { "$($_.Name)=$($_.Count)" }) -join ", "
Write-Output "Voies        : $resumeVoies"

$replis = $tirs | Where-Object { $_.voie -ne "ok" }
$vides = $tirs | Where-Object { $_.n -eq 0 }

# --- Durees --------------------------------------------------------------
$durees = ($tirs | ForEach-Object { $_.us / 1000.0 }) | Sort-Object
$mediane = $durees[[int]($durees.Count / 2)]
$p95 = $durees[[int]([math]::Floor($durees.Count * 0.95))]
Write-Output "Duree        : mediane $([math]::Round($mediane,1)) ms, p95 $([math]::Round($p95,1)) ms, max $([math]::Round($durees[-1],1)) ms"

# --- Changements de contenu ---------------------------------------------
# Claude Code ne redessine que si la chaine differe de la precedente (methode
# `#w` de la classe CFc : `if (this.#r.text === e) return`). Le nombre de
# signatures changees est donc le nombre de redessins reellement demandes.
$changements = 0
for ($i = 1; $i -lt $tirs.Count; $i++) {
    if ($tirs[$i].sig -ne $tirs[$i - 1].sig) { $changements++ }
}
Write-Output "Redessins    : $changements sur $($tirs.Count - 1) lancements ont change la ligne"

# --- Trous ---------------------------------------------------------------
$trous = @()
for ($i = 1; $i -lt $tirs.Count; $i++) {
    $ecart = $tirs[$i].t - $tirs[$i - 1].t
    if ($ecart -gt $SeuilTrouMs) {
        $trous += [pscustomobject]@{
            Heure   = [DateTimeOffset]::FromUnixTimeMilliseconds($tirs[$i - 1].t).ToLocalTime().ToString('HH:mm:ss.fff')
            EcartMs = $ecart
        }
    }
}

Write-Output "Trous        : $($trous.Count) intervalle(s) au-dela de $SeuilTrouMs ms"
if ($trous.Count -gt 0) {
    $aMontrer = if ($Detail) { $trous } else { $trous | Select-Object -First 10 }
    foreach ($trou in $aMontrer) {
        Write-Output "  a $($trou.Heure) : $([math]::Round($trou.EcartMs / 1000.0, 2)) s sans lancement"
    }
    if (-not $Detail -and $trous.Count -gt 10) {
        Write-Output "  ... et $($trous.Count - 10) autres (-Detail pour tout voir)"
    }
}

# --- Fenetre autour d'un clignotement observe ----------------------------
if ($Autour) {
    [datetime]$cible = [datetime]::MinValue
    $lisible = [datetime]::TryParse($Autour, [ref]$cible)
    if (-not $lisible) {
        Write-Output ""
        Write-Output "Heure illisible : '$Autour'. Format attendu HH:mm:ss."
    } else {
        # Une heure nue se rapporte au jour du journal, pas a aujourd'hui.
        if ($cible.Date -eq [DateTime]::Today.Date -and $Autour -notmatch '\d{4}') {
            $cible = $debut.Date.Add($cible.TimeOfDay)
        }
        $centre = [DateTimeOffset]::new($cible).ToUnixTimeMilliseconds()
        $marge = $FenetreS * 1000
        $fenetre = $tirs | Where-Object { [math]::Abs($_.t - $centre) -le $marge }

        Write-Output ""
        Write-Output "=== Lancements autour de $($cible.ToString('HH:mm:ss')) (+/- $FenetreS s) ==="
        if ($fenetre.Count -eq 0) {
            Write-Output "  AUCUN lancement dans cette fenetre."
            Write-Output "  -> la ligne n'a pas ete recalculee a cet instant ; si elle a disparu, c'est"
            Write-Output "     qu'un tir a manque et que Claude Code a recu un resultat indefini."
        } else {
            $precedente = $null
            foreach ($tir in $fenetre) {
                $heure = [DateTimeOffset]::FromUnixTimeMilliseconds($tir.t).ToLocalTime().ToString('HH:mm:ss.fff')
                $marqueur = if ($null -ne $precedente -and $tir.sig -ne $precedente) { "  <- ligne changee" } else { "" }
                # Le champ « panique » n'existe que sur un tir qui a panique,
                # depuis le 04/09/2026 : lieu et message retenus par le hook.
                $panique = if ($null -ne $tir.PSObject.Properties['panique']) { "  panique=$($tir.panique)" } else { "" }
                Write-Output ("  {0}  {1,6:N1} ms  voie={2}  n={3}  sig={4}{5}{6}" -f `
                        $heure, ($tir.us / 1000.0), $tir.voie, $tir.n, $tir.sig.Substring(0, 8), $marqueur, $panique)
                $precedente = $tir.sig
            }
        }
    }
}

# --- Verdict -------------------------------------------------------------
Write-Output ""
Write-Output "=== Lecture ==="
if ($vides.Count -gt 0) {
    Write-Output "Le programme a ecrit $($vides.Count) ligne(s) VIDE : c'est lui qui efface la ligne."
    Write-Output "  -> anomalie a corriger dans statusline-rs, l'invariant est viole."
} elseif ($replis.Count -gt 0) {
    Write-Output "$($replis.Count) lancement(s) sont passes par un repli ($(($replis | Group-Object voie | ForEach-Object { $_.Name }) -join ', '))."
    Write-Output "  -> la ligne s'est reduite au nom du modele, ce qui se voit comme une disparition partielle."
    # Depuis le 04/09/2026, un tir en voie « panic » porte le lieu et le message
    # de la panique : c'est ce qui evite de relire le code pour la retrouver.
    $paniques = @($replis |
        Where-Object { $null -ne $_.PSObject.Properties['panique'] } |
        ForEach-Object { $_.panique } |
        Select-Object -Unique)
    if ($paniques.Count -gt 0) {
        Write-Output "  -> panique(s) retenue(s) par le hook :"
        foreach ($p in ($paniques | Select-Object -First 5)) {
            Write-Output "     $p"
        }
        if ($paniques.Count -gt 5) {
            Write-Output "     ... et $($paniques.Count - 5) autre(s) distincte(s)"
        }
    }
} elseif ($trous.Count -gt 0) {
    $parMinute = [math]::Round($trous.Count / [math]::Max($duree / 60.0, 0.01), 1)
    Write-Output "Aucun lancement muet, mais $($trous.Count) intervalle(s) trop long(s), soit $parMinute par minute."
    Write-Output "  -> deux causes possibles, que le journal ne separe pas : un tir qui n'a jamais"
    Write-Output "     demarre (spawn_failed, et la ligne s'efface), ou un tir tue par le suivant"
    Write-Output "     avant d'avoir pu ecrire (et la ligne, elle, ne bouge pas)."
    Write-Output "  -> comparer ce rythme a celui du clignotement observe : s'ils different, la"
    Write-Output "     ligne de statut n'explique pas le symptome. -Autour <HH:mm:ss> tranche."
} else {
    Write-Output "Cadence reguliere, aucun lancement muet, aucun repli."
    Write-Output "  -> la ligne de statut n'est jamais restee vide : le clignotement observe vient"
    Write-Output "     du redessin de l'interface par le terminal, pas de ce programme."
    Write-Output "     Le nombre de redessins ci-dessus dit combien Claude Code en a demande."
}
