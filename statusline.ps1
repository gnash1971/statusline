<#
.SYNOPSIS
    Ligne de statut Claude Code : abonnement, modèle, effort, emplacement,
    contexte et fenêtres de limitation.

.DESCRIPTION
    GELÉ LE 19/09/2026 — ce script n'évolue plus. La ligne de statut active est
    statusline.exe (crate statusline-rs), qui depuis sa version 2.0.0 écrit
    deux lignes en capsules, avec des fonds et des caps que ce script ne
    décrit pas. Il reste l'oracle de test-statusline.ps1 pour la seule sortie
    sous NO_COLOR, où l'exe rend octet pour octet la ligne décrite ici — le
    harnais le compare avec -IgnorerCouleur —, et le repli si le binaire
    venait à manquer. Décision et raisons : .claude\CHANTIER-statusline-capsule.md
    (Q2) et statusline-rust.md §23.

    Une exception au gel, le 23/09/2026 : deux retouches demandées par
    l'utilisateur changent la sortie sous NO_COLOR, et y ont donc été portées
    à l'identique pour que l'oracle reste comparable — le segment de version
    est retiré, et un chemin trop profond se replie en « …\feuille » au lieu
    de « racine\…\feuille ».

    Claude Code transmet sur l'entrée standard un objet JSON décrivant la
    session en cours, et affiche telle quelle la ligne écrite sur la sortie
    standard.

    Le contrat d'entrée (Claude Code 2.1.226) porte notamment :
      - model.display_name                    : nom lisible, ex. « Opus 5 »
      - model.id                              : identifiant, ex. « claude-fable-5-1 » —
                                                en donne la famille, voir Get-FamilleModele
      - effort.level                          : low|medium|high|xhigh|max
      - fast_mode                             : mode rapide actif
      - thinking.enabled                      : réflexion étendue active
      - workspace.current_dir                 : répertoire de travail
      - workspace.project_dir                 : répertoire de lancement
      - context_window.used_percentage        : contexte occupé, en pourcentage
      - rate_limits.<fenêtre>.used_percentage : consommation, en pourcentage
      - rate_limits.<fenêtre>.resets_at       : remise à zéro, en secondes Unix

    Les fenêtres de limitation sont au nombre de deux, « five_hour » et
    « seven_day ». Chacune peut être absente indépendamment de l'autre, et
    aucune n'est renseignée avant la première réponse de l'API.

    L'abonnement ne vient pas de ce payload, et pour une raison radicale : le
    contrat n'en porte rien du tout. Les trente-quatre champs relevés le 26/08/2026 sur la
    version 2.1.246 sont muets là-dessus, et la valeur se lit donc dans
    « ~/.claude.json » — voir Get-Abonnement. Le même fichier porte, depuis le
    03/09/2026, la fenêtre hebdomadaire propre au modèle que /usage relève —
    voir Get-FenetreModele.

    Chaque fenêtre affiche aussi son rythme de consommation, calculé sur une
    pente réellement mesurée depuis la première fois que le script a vu cette
    fenêtre-là. Cette mesure est la raison d'être des deux champs d'ancrage du
    cache : voir Get-Ancrage et Format-Rythme.

    Ce rythme se tait plus souvent qu'il ne parle, et notamment tant que la
    durée à projeter dépasse trop largement la durée observée. Une projection
    hebdomadaire tirée de quelques minutes d'activité annoncerait un épuisement
    imminent à 5 % de consommation : voir $RatioMaxExtrapolation.

    Sortie type :
    « Pro Opus 5 xhigh PY_xl (main) ctx 34% · 5h ▒ 29%
      → 61% 15:00 · 7j ▒ 41% », et sur la fenêtre qui va au plafond
    « 5h █ 80% épuisé 14:10 15:00 ».

    La ligne se lit sur trois rangs, et deux dispositifs les distinguent :

      - l'espacement : « · » sépare les segments d'un même groupe, l'espace
        simple les morceaux d'un même segment — et les trois groupes entre eux,
        depuis que la pastille de l'emplacement borne le sien. Les groupes sont
        ceux que la ligne décrivait déjà sans le dire : ce qui tourne, où l'on
        est, ce que la session consomme ;
      - la teinte : le chrome — libellés, unités, heures de remise à zéro,
        effort, et le séparateur de segment — passe en gris, les valeurs gardent
        la couleur du thème. Voir Format-Mesure, qui compose ce couple pour tous
        les compteurs. Les marqueurs de session, eux, ont leur teinte propre :
        voir $CodeMarqueur.

    Le premier dispositif ne coûte aucune couleur et survit donc à NO_COLOR,
    sauf sur le rang des groupes : leur séparation tenait à une barre « │ »
    jusqu'au 26/08/2026 au soir, et repose désormais sur le fond de la pastille.
    Voir $SeparateurGroupe.

    Deux segments portent un fond coloré, et deux seulement : l'abonnement en
    tête, dans la teinte de marque du produit, et l'emplacement, pour que l'œil
    retrouve d'où l'on parle. Ce sont les deux informations les plus stables de
    la ligne ; les mesures, elles, gardent la coloration des paliers pour elles
    seules. Voir $CodePastilleAbonnement et $CodePastille.

    Les pourcentages franchissent deux paliers de coloration, ambre puis rouge,
    parce que 81 % et 99 % n'appellent pas la même réaction. Ils sont par
    ailleurs cadrés à droite sur une largeur fixe : un compteur qui passe de
    9 % à 10 % ne décale plus tout ce qui le suit.

    Organisation du fichier, dans l'ordre :

      1. réglages          — seuils, couleurs, noms de fichiers ;
      2. sortie            — écriture de la ligne et coloration ;
      3. conversions       — lecture défensive des valeurs du payload ;
      4. état sur disque   — cache de la fenêtre et journal de diagnostic ;
      5. lectures disque   — branche Git, abonnement ;
      6. segments          — un producteur par morceau de la ligne ;
      7. assemblage        — composition de la ligne complète ;
      8. programme principal.

    Deux conventions de nommage se répondent dans les sections 6 et 7 :
    « Format-* » met en forme une valeur déjà extraite et n'a aucun effet de
    bord, « Get-Segment* » produit un morceau de la ligne à partir du payload
    et rend $null quand l'information manque — le segment se retire alors de
    lui-même, sans laisser de séparateur suivi de rien.

    Rien n'est jamais fatal. Les valeurs du payload passent par
    ConvertTo-Pourcent et ConvertTo-Instant, qui rendent $null plutôt que de
    lever ; le programme principal, en dernier recours, écrit le nom du modèle
    seul. Une sortie vide effacerait la ligne dans l'interface, ce qui est pire
    qu'une information partielle : c'est la raison d'être de ces filets.
    Le mode strict est volontairement absent, pour la même raison.

    Ces filets se referment sur un invariant unique, tenu par la section 8 :
    le script écrit **toujours** une ligne non blanche et rend **toujours** le
    code 0. Un payload vide, un JSON illisible, un assemblage qui lève : tous
    ces chemins écrivent quelque chose. Se taire n'est jamais un affichage
    minimal, c'est un effacement — Claude Code ne garde pas le texte précédent.

    Le coût cumulé (cost.total_cost_usd) était affiché ici auparavant. Il reste
    alimenté par Claude Code, mais sur un abonnement il ne correspond à aucune
    facturation : la fenêtre de 5 heures est la contrainte réelle.

.NOTES
    Câblé via la clé « statusLine » de ~/.claude/settings.json.
    Copie versionnée : <projet>\.claude\user-config\statusline.ps1, déployée
    par sync-user-config.ps1.
    Non-régression : <projet>\.claude\user-config\test-statusline.ps1, à passer
    avant tout déploiement.
#>

# ===========================================================================
# 1. Réglages
# ===========================================================================

# Séparateur inséré entre deux segments d'un même groupe. Rang intermédiaire des
# trois : voir $SeparateurGroupe au-dessus — qui ne s'écrit plus depuis le
# 26/08/2026 au soir — et $SeparateurInterne en dessous.
#
# C'est donc le seul des trois rangs à porter un glyphe, les deux autres se
# contentant d'un espace : l'un parce que la pastille borne son groupe elle-même,
# l'autre parce qu'un segment n'a pas à s'articuler plus fort que cela.
$Separateur = " · "

# Séquence SGR du séparateur de segment — 26/08/2026, second lot.
#
# Ce point médian sortait sans aucune séquence jusqu'au matin du 26/08/2026, donc
# dans la couleur pleine du thème : le séparateur du rang intermédiaire était par
# là même l'élément le plus lumineux de la zone des compteurs. Il a d'abord
# rejoint le chrome au gris 240 que portait alors la barre de groupe.
#
# Repris, donc, à 245. Partager 240 avec la barre donnait deux rangs pour une
# seule teinte — la hiérarchie était écrite dans les constantes et invisible à
# l'écran. Le point se lit désormais au rang des libellés qu'il sépare,
# c'est-à-dire à la valeur de $CodeAttenue.
#
# La barre est tombée le soir même, et le point est resté seul de son espèce :
# voir $SeparateurGroupe. La constante n'a plus d'écart à tenir avec une autre
# ponctuation, seulement le rang du chrome à épouser.
$CodeSeparateur = "38;5;245"

# Séparateur interne à un segment composite : les morceaux d'une même fenêtre
# de limitation. Il est volontairement plus discret que $Separateur, qui marque
# le rang supérieur. Ces deux rangs-là partageaient auparavant le même « · », si
# bien que « 5h 85% · reset 21:30 · 7j 78% » se lisait comme trois items de même
# niveau et que rien ne rattachait une heure à sa fenêtre.
#
# Cette distinction ne dépend d'aucune couleur : elle survit à NO_COLOR, là où
# la hiérarchie portée par $CodeAttenue disparaît.
$SeparateurInterne = " "

# Séparateur de groupe, le rang le plus élevé des trois — réduit à l'espace
# simple le 26/08/2026 au soir.
#
# La ligne compte trois familles : ce qui tourne (abonnement, modèle),
# où l'on est (emplacement), ce que la session consomme (contexte et fenêtres).
# Ce rang a porté du 25/08/2026 au 26/08/2026 une barre « │ » — U+2502, présent
# dans Consolas, contrairement aux séparateurs powerline qui exigeraient une
# police à glyphes.
#
# Elle est tombée parce que le groupe du milieu se borne tout seul. Trois
# familles ne font que deux jonctions, et l'emplacement est des deux côtés : la
# barre n'a donc jamais séparé que la pastille de ses voisins, là où le fond
# marque déjà le début et la fin du segment au caractère près. Quatre colonnes —
# deux par jonction, l'espace restant — pour redire ce qu'un aplat de couleur
# disait mieux, sur une ligne dont chaque retouche de la journée a consisté à en
# rendre.
#
# L'espace qui reste n'est pas le rang inférieur remonté d'un cran : c'est la
# trace que le rang supérieur s'écrit désormais par le fond du groupe qu'il
# borne, comme le rang inférieur s'écrit par l'espace à l'intérieur d'un segment.
# Seul le rang du milieu garde un glyphe, voir $Separateur.
#
# Le prix se paie sous NO_COLOR, où le fond s'en va : la ligne y perd la
# séparation de ses familles et ne garde que le « · » de ses segments. C'est le
# mode où toute la hiérarchie des teintes est éteinte et où les deux pastilles
# rendent leur fond — un affichage dégradé, jamais celui qu'on regarde.
$SeparateurGroupe = " "

# Libellé affiché quand le modèle est inconnu, et repli ultime sur erreur.
$ModeleParDefaut = "Claude"

# Configuration de Claude Code, relative au profil utilisateur : c'est là que se
# lit l'abonnement du compte connecté, le payload n'en disant rien. La variable
# CLAUDE_STATUSLINE_CONFIG désigne un autre chemin, absolu, sur un poste
# installé ailleurs. Voir Get-Abonnement.
$CheminConfigRelatif = ".claude.json"

# Types d'organisation et libellés d'abonnement correspondants.
#
# Les quatre clés sont relevées dans le bundle JavaScript de « claude.exe », où
# Claude Code tient la même correspondance pour son propre usage :
# new Map([["claude_max","max"],["claude_pro","pro"],
# ["claude_enterprise","enterprise"],["claude_team","team"]]). Elles forment donc
# l'ensemble complet à la version 2.1.246 — et jusqu'à la prochaine, d'où le
# repli de ConvertTo-LibelleLisible sur toute valeur hors table.
#
# Table ordonnée plutôt que dictionnaire : la recherche se fait à la casse près,
# ce qu'un accès par clé PowerShell ne permet pas.
#
# Le mot « Claude » est tombé le 26/08/2026, second lot de retouches. Il occupait
# sept colonnes en tête de ligne pour ne rien distinguer : dans une ligne de
# statut de Claude Code, aucun autre éditeur ne dispute le nom. C'est mot pour
# mot l'argument qui avait réduit « Claude Code v » au « v » seul, appliqué au
# segment voisin — celui de version, retiré depuis, le 23/09/2026.
#
# ConvertTo-LibelleLisible élague le même préfixe sur les valeurs hors table,
# faute de quoi un abonnement inventé après cette version s'afficherait
# « Claude Ultra Plus » à côté d'un « Pro » — la table dirait une chose et le
# repli une autre.
$Abonnements = @(
    [pscustomobject]@{ Cle = "claude_pro"; Libelle = "Pro" },
    [pscustomobject]@{ Cle = "claude_max"; Libelle = "Max" },
    [pscustomobject]@{ Cle = "claude_team"; Libelle = "Team" },
    [pscustomobject]@{ Cle = "claude_enterprise"; Libelle = "Enterprise" }
)

# Paliers d'un abonnement Max, et le suffixe accolé au libellé.
#
# Le palier vit dans un champ distinct du type d'organisation, « 5x » et « 20x »
# n'étant pas deux abonnements mais deux volumes du même. Les deux clés viennent
# du même relevé que $Abonnements, où Claude Code les compare pour décider ce
# qu'il propose à la vente.
$PaliersMax = @(
    [pscustomobject]@{ Cle = "default_claude_max_5x"; Suffixe = "5x" },
    [pscustomobject]@{ Cle = "default_claude_max_20x"; Suffixe = "20x" }
)

# Type d'organisation qui, seul, accepte un suffixe de palier.
$TypeAbonnementMax = "claude_max"

# Seuils de mise en alerte, en pourcent. Distincts parce que les trois
# compteurs n'appellent pas la même réaction : le contexte se compacte, la
# fenêtre de 5 heures se subit une poignée d'heures, celle de 7 jours se subit
# une semaine — d'où un seuil hebdomadaire plus bas, qui laisse le temps de
# lever le pied avant d'y être.
#
# Chaque compteur porte deux paliers, et non un seul : franchir le premier
# appelle à lever le pied, franchir le second à s'arrêter. La coloration
# passait auparavant d'un coup du neutre à l'alerte, si bien que 81 % et 99 %
# se ressemblaient — alors que le premier laisse une marge de manœuvre et que
# le second n'en laisse aucune.
#
# Les seuils d'origine gardent leur valeur et deviennent le premier palier. Le
# second est posé dix points plus haut, assez près du plafond pour ne se
# déclencher que quand il n'y a plus rien à arbitrer.
$SeuilContexte = 80
$SeuilCritiqueContexte = 90
$SeuilFenetre5h = 80
$SeuilCritiqueFenetre5h = 90
$SeuilFenetre7j = 75
$SeuilCritiqueFenetre7j = 85

# Fenêtres de limitation affichées, dans l'ordre. « Cle » est celle du payload
# et du cache, « Libelle » le préfixe affiché.
#
# ResetToujours réserve l'affichage de l'heure de remise à zéro à la fenêtre de
# 5 heures. À l'échelle de la semaine, cette heure n'est actionnable qu'une fois
# le plafond en vue : Format-Fenetre l'ajoute alors de lui-même, au moment où
# elle sert vraiment, et la ligne reste courte le reste du temps.
#
# Deux drapeaux font suivre le modèle à une fenêtre (03/09/2026) :
# SeuilsSelonModele abaisse ses seuils sous un modèle plus cher qu'Opus, et
# AncrageSelonModele réancre son rythme quand la famille du modèle change —
# voir $FacteursModele juste en dessous. Seule la fenêtre de 7 jours du payload
# déclare les deux : c'est elle qui se subit une semaine, et son ancrage
# traverse les sessions, donc les changements de modèle. Celle de 5 heures se
# réancre d'elle-même à chaque remise à zéro, quelques heures au plus après un
# changement ; la faire taire cinquante minutes à chaque bascule coûterait plus
# qu'une pente un peu datée. Ses seuils restent ceux d'Opus, et son cache ne
# porte pas de famille.
$Fenetres = @(
    [pscustomobject]@{
        Cle = "five_hour"; Libelle = "5h"
        Seuil = $SeuilFenetre5h; SeuilCritique = $SeuilCritiqueFenetre5h
        ResetToujours = $true
        SeuilsSelonModele = $false
        AncrageSelonModele = $false
    },
    [pscustomobject]@{
        Cle = "seven_day"; Libelle = "7j"
        Seuil = $SeuilFenetre7j; SeuilCritique = $SeuilCritiqueFenetre7j
        ResetToujours = $false
        SeuilsSelonModele = $true
        AncrageSelonModele = $true
    }
)

# Fenêtre hebdomadaire propre au modèle, relevée par /usage et lue dans
# ~/.claude.json — 03/09/2026, au soir. Voir Get-FenetreModele.
#
# Même libellé que la fenêtre globale de 7 jours : les deux se disputent la
# même place sur la ligne, et c'est la plus remplie qui l'occupe — celle qui
# bornera la session la première. Sur ce poste le 03/09/2026 : 28 % pour Fable
# contre 14 % pour tous les modèles, et /usage dit 28.
#
# Ses seuils sont ceux d'Opus, sans adaptation : ce budget n'est rempli que par
# le modèle dont il porte le nom, et son pourcentage dit déjà tout du prix du
# jeton. Son ancrage, lui, suit la famille — une fenêtre Sonnet n'est pas une
# fenêtre Fable, et la clé de cache est la même pour toutes.
$FenetreModele = [pscustomobject]@{
    Cle = "seven_day_modele"; Libelle = "7j"
    Seuil = $SeuilFenetre7j; SeuilCritique = $SeuilCritiqueFenetre7j
    ResetToujours = $false
    SeuilsSelonModele = $false
    AncrageSelonModele = $true
}

# Clé de ~/.claude.json sous laquelle Claude Code persiste la réponse de
# /api/oauth/usage, et genre des entrées de utilization.limits[] qui portent
# une fenêtre propre à un modèle. Relevés dans le bundle de claude.exe 2.1.259
# et sur le fichier du poste, le 03/09/2026.
$CleUsagePersiste = "cachedUsageUtilization"
$GenreFenetreModele = "weekly_scoped"

# Genre de l'entrée de utilization.limits[] qui porte la fenêtre hebdomadaire
# de tous les modèles — la même que rate_limits.seven_day du payload, vue au
# même instant que la fenêtre du modèle. C'est le point de référence de la
# dérive — 05/09/2026, voir Get-DeriveModele.
$GenreFenetreGlobale = "weekly_all"

# Âge au-delà duquel un relevé de /usage s'affiche marqué « ~ » et sans rythme,
# en secondes. Une heure, parce que c'est la borne de Claude Code lui-même :
# passé ce délai il ne relit plus la valeur persistée. La ligne, elle, continue
# de l'afficher tant que la fenêtre n'a pas expiré — c'est le seul chiffre
# disponible, et une valeur datée vaut mieux qu'un retour muet à la fenêtre
# globale, qui se lirait comme une chute de 28 à 14 %. Le relevé n'est rafraîchi
# qu'à l'ouverture de /usage, mesuré sur le poste : le « ~ » dira donc, le plus
# souvent, qu'il est temps de l'ouvrir.
$AgeMaxUsagePersiste = 3600

# Facteurs de consommation par famille de modèle, rapportés à Opus — 03/09/2026.
#
# Le budget hebdomadaire est unique quel que soit le modèle, mais il se vide au
# prix du jeton : à travail égal, Fable 5.1 consomme deux fois ce qu'Opus 5
# consomme. Les valeurs sont le rapport des tarifs à ceux d'Opus 5, relevé sur
# la grille du 03/09/2026 — le rapport est le même en entrée et en sortie :
# Fable 5.1 et Mythos 5.1 à 10 $ / 50 $ le million de jetons, Opus 5 à
# 5 $ / 25 $, Sonnet 5 à 2 $ / 10 $, Haiku 4.5 à 1 $ / 5 $.
#
# La famille se lit dans model.id — « claude-fable-5-1 » —, ou à défaut dans
# model.display_name : voir Get-FamilleModele. Un modèle hors table vaut 1, et
# la ligne se comporte alors comme avant.
#
# Deux effets, réservés aux fenêtres qui déclarent SelonModele :
#   - les seuils s'abaissent sous un facteur supérieur à 1, de sorte que
#     l'alerte laisse le même temps de réaction. Un facteur inférieur à 1 ne
#     les relève jamais — voir ConvertTo-SeuilAdapte pour la raison ;
#   - le rythme se réancre quand la famille change, la pente mesurée sous l'une
#     ne disant rien de ce que l'autre va consommer — voir Get-Ancrage. Là,
#     c'est la famille qui compte et non la valeur du facteur : Opus 4.8 et
#     Opus 5 partagent la leur.
#
# Le payload ne porte aucune fenêtre propre au modèle, et ce n'est pas une
# supposition : la capture du 03/09/2026 sur la 2.1.259, en session Fable, ne
# montre que « five_hour » et « seven_day ». Le bundle de claude.exe tient bien
# des fenêtres « seven_day_opus », « seven_day_sonnet » et une « Fable limit »
# (« seven_day_overage_included »), mais les réserve à l'écran /usage. D'où un
# facteur plutôt qu'une lecture.
$FacteursModele = @(
    [pscustomobject]@{ Famille = "fable";  Facteur = 2.0 },
    [pscustomobject]@{ Famille = "mythos"; Facteur = 2.0 },
    [pscustomobject]@{ Famille = "opus";   Facteur = 1.0 },
    [pscustomobject]@{ Famille = "sonnet"; Facteur = 0.4 },
    [pscustomobject]@{ Famille = "haiku";  Facteur = 0.2 }
)

# Marqueurs accolés au modèle. Ils ne signalent que l'état inhabituel : mode
# rapide actif, réflexion étendue coupée. Voir Get-SegmentModele.
$LibelleModeRapide = "fast"
$LibelleSansReflexion = "sans réflexion"

# Durée d'observation minimale avant d'oser une projection, en minutes. En
# deçà, la pente se calcule sur un ou deux points de pourcentage — ils sont
# entiers dans le cache — et la projection tiendrait de la divination.
#
# Ce plancher est absolu, et ne suffit pas à lui seul : voir juste en dessous.
$SpanMinimumRythme = 10

# Facteur d'extrapolation maximal : rapport toléré entre la durée projetée et
# la durée réellement observée. Au-delà, le rythme se tait.
#
# Le plancher en minutes traite les deux fenêtres de la même façon, alors
# qu'elles n'extrapolent pas du tout à la même échelle. Dix minutes suffisent
# à dire quelque chose de trois heures ; les mêmes dix minutes déroulées sur
# sept jours multiplient la mesure par mille, et une rafale d'activité y
# devient un épuisement annoncé pour le lendemain — vu en conditions réelles à
# 5 % de consommation hebdomadaire.
#
# Borner le rapport plutôt que la durée règle les deux fenêtres d'une seule
# règle, puisque c'est bien l'ampleur de l'extrapolation, et non sa durée
# absolue, qui décide de ce que la pente vaut. À 6 : la fenêtre de 5 heures
# demande une demi-heure d'observation quand il en reste trois, celle de
# 7 jours une quinzaine d'heures quand il en reste quatre jours. La projection
# hebdomadaire réapparaît donc dans la seconde moitié de sa fenêtre, quand
# l'ancrage a vieilli — c'est-à-dire quand elle devient actionnable.
$RatioMaxExtrapolation = 6

# Préfixes du segment de rythme : projection à la fin de fenêtre, et heure
# d'épuisement quand le plafond arrive avant la remise à zéro. Sans espace
# final : l'espace qui les sépare de leur valeur est posé par Format-Mesure,
# hors de la séquence de couleur.
$PrefixeProjection = "→"
$PrefixeEpuisement = "épuisé"

# Marqueurs accolés au pourcentage d'une fenêtre, à gauche de la valeur, quand
# elle n'est pas une mesure fraîche. « ~ » : valeur mémorisée — cache de la
# ligne avant la première réponse, ou relevé de /usage vieux de plus d'une
# heure ; ancienne, pas fausse. « ≈ » — 05/09/2026 : valeur estimée, la fenêtre
# du modèle avancée de ce que la globale du payload a pris depuis le relevé —
# voir Get-DeriveModele. Ce n'est plus une mesure, et le marqueur le dit ; le
# rythme se tait, comme sous « ~ ».
$MarqueurMemorisee = "~"
$MarqueurEstimee = "≈"

# Unité des quatre compteurs — contexte, deux fenêtres, projection. Passée du
# côté du chrome le 25/08/2026 ; le littéral est ici pour que ce déplacement
# n'ait qu'un seul point d'entrée. Voir Format-Mesure.
$UnitePourcent = "%"

# Séquences SGR des deux paliers, en palette 256 explicite depuis le
# 25/08/2026 — elles étaient auparavant écrites « 1;33 » et « 1;31 ».
#
# Le « 1 » de tête ne faisait pas qu'épaissir : sur Windows Terminal, il bascule
# la couleur ANSI vers sa variante vive. L'alerte tombait donc sur un jaune
# #F5F543 qui vibre à travers un fond translucide, en Consolas corps 8 de
# graisse light — la couleur se voyait au prix du nombre qu'elle portait.
#
# 214 (#ffaf00) est un ambre franc, lisible sur fond sombre comme clair. Le gras
# subsiste sur le seul palier critique, où l'épaisseur est un signal et non
# l'effet de bord d'une notation.
#
# Comme $CodePastille, ces deux valeurs doivent rester identiques à leurs
# jumelles côté Rust : les cas d'alerte du harnais les exercent.
$CodeAlerte = "38;5;214"
$CodeCritique = "1;38;5;203"

# Séquence SGR des marqueurs de session — 25/08/2026.
#
# « 1M », « fast » et « sans réflexion » sortaient dans la couleur pleine du
# thème, celle du nom du modèle. Ils se lisaient donc comme une partie de ce
# nom — « Opus 5 1M » — alors qu'ils décrivent un état de session et ne
# paraissent que sur l'inhabituel.
#
# 109 (#87afaf) est un cyan sourd : plus lumineux que le chrome, moins que le
# nom, ce qui est exactement leur rang. Il ne dispute rien à l'ambre du palier
# d'alerte ni au vert de la pastille. L'effort, lui, garde son gris : il qualifie
# le modèle et se consulte, il n'annonce pas un écart.
#
# Comme $CodePastille, cette valeur doit rester identique à sa jumelle côté
# Rust : les cas du harnais qui portent « fast_mode » l'exercent.
$CodeMarqueur = "38;5;109"

# Séquence SGR des libellés : intensité réduite. Elle ne porte que le chrome —
# nom des compteurs, « reset », version, effort — pour que les valeurs
# ressortent d'elles-mêmes en intensité normale, sans qu'aucune couleur
# supplémentaire soit nécessaire.
#
# Un gris explicite de la palette 256 couleurs, et non « 2 » (intensité
# réduite). « 2 » avait pour lui d'être relatif à la couleur courante, donc
# valable sur n'importe quel fond ; mais plusieurs terminaux le rendent
# faiblement, voire l'ignorent, et sur un fond sombre où le texte est déjà d'un
# gris clair l'écart devenait invisible : la hiérarchie ne se voyait plus.
#
# 245 se situe au milieu de la rampe de gris (232 à 255), assez bas pour se
# détacher du texte normal sur fond sombre, assez haut pour rester lisible sur
# fond clair. Contrepartie assumée : la couleur est désormais absolue, et un
# thème très inhabituel pourrait la rapprocher du fond. Repasser à "2" suffit
# à revenir au comportement relatif.
$CodeAttenue = "38;5;245"

# Séquence SGR de la pastille : fond vert profond, texte vert clair.
#
# Seul endroit de la ligne où une couleur de fond est posée, et il est choisi
# pour cela. Un fond courant sur toute la ligne aurait coûté trois choses : le
# « ESC[0m » qui referme chaque fragment remet le fond à zéro, si bien qu'il
# aurait fallu passer tous les segments à des remises à zéro sélectives ; le
# gris et l'ambre ont été réglés pour tenir sur n'importe quel fond de thème,
# ce qui aurait cessé d'être vrai ; et un vert permanent aurait dit « tout va
# bien » jusque sur une ligne à « 5h 95% ». Cantonné au segment d'emplacement,
# le fond accroche l'œil sans toucher à la zone des mesures, et se referme
# aussitôt.
#
# Le sens du contraste suit celui du terminal, et il a été retourné le
# 25/08/2026. La pastille était née claire — fond 157 (#afffaf), texte 238 — sur
# l'hypothèse d'un terminal clair, écrite telle quelle ici. La configuration du
# poste dit le contraire : Windows Terminal y est en schéma Dark+ (fond
# #1E1E1E), avec opacity 80 et acrylic. Un aplat presque blanc y devenait le
# point le plus lumineux de la ligne, plus que le rouge du palier critique.
#
# D'où l'inversion : fond 22 (#005f00) et texte 157. Le contraste reste d'environ
# 10:1 et la pastille garde sa fonction d'ancre, sans disputer la hiérarchie aux
# paliers d'alerte.
#
# Ce fichier n'est plus lancé par Claude Code, mais il reste l'oracle du harnais :
# la valeur doit rester identique à CODE_PASTILLE côté Rust, faute de quoi les
# 81 cas divergent sur le seul segment d'emplacement — qu'ils exercent presque
# tous. C'est la seule catégorie de changement qui se porte des deux côtés : une
# constante d'affichage, à l'identique, ne coûte pas l'oracle. Une évolution de
# logique, elle, reste côté Rust.
$CodePastille = "48;5;22;38;5;157"

# Séquence SGR de la pastille de tête : la couleur du corps de clawd sur un fond
# profond de la même teinte, en couleurs vraies.
#
# Relevée dans le bundle de claude.exe 2.1.246, où les thèmes la nomment
# clawd_body : rgb(215,119,87). Les six thèmes en couleurs vraies portent la même
# valeur — clair, sombre et daltonisés confondus ; seuls les deux thèmes en seize
# couleurs se rabattent sur redBright. Ce n'est donc pas une couleur de thème
# mais une couleur de marque.
#
# Elle a d'abord peint le crabe « (V)°°(V) » posé en tête de ligne, du 26/08/2026
# au soir du même jour. Le logo est parti — huit colonnes qui ne disaient rien de
# la session — et l'abonnement a pris sa place, avec sa pastille et sa teinte. Ce
# que la marque désignait de loin, un mot le dit : « Pro » en tête d'une ligne de
# statut de Claude Code n'a pas besoin d'un crabe à côté pour qu'on sache de quel
# produit il parle. La couleur, elle, n'a aucune raison de changer : c'est celle
# du produit, à l'endroit où la ligne le nomme.
#
# La ligne est en palette 256 partout ailleurs. Cette pastille passe en couleurs
# vraies parce qu'il ne s'agit pas d'y tenir sur n'importe quel thème mais de
# rendre une couleur précise ; 173 (#d7875f) en serait l'approximation en palette
# 256.
#
# Fond rgb(48,14,0), soit #300E00 : la teinte du corps ramenée à sa valeur la
# plus basse, restée dans la même famille chaude plutôt que rabattue au noir. Le
# texte, lui, ne bouge pas — rgb(215,119,87) est la couleur de marque, et la
# fidélité prime. Cela borne le contraste : contre un noir absolu, la couleur du
# corps ne dépasserait pas 6,7:1 ; #300E00 en rend 5,6:1, au-dessus du seuil AA
# de 4,5:1, et garde au plan sa chaleur.
#
# Comme $CodePastille, cette valeur doit rester identique à sa jumelle côté
# Rust : l'abonnement paraît sur tous les cas du harnais où la configuration est
# lisible.
$CodePastilleAbonnement = "48;2;48;14;0;38;2;215;119;87"

# Largeur du champ de valeur, unité comprise, cadré à droite.
#
# Sans ce calage, la ligne se décale à chaque rafraîchissement : « ctx 9% »
# devenant « ctx 10% », tout ce qui suit glisse d'un caractère, et l'œil doit
# retrouver des repères qui n'ont pourtant pas changé de nature. Le « ~ » des
# valeurs mémorisées est compté dans le champ, ce qui aligne « ~11% » sur
# « 34% ».
#
# Étalonnée sur « 99% » depuis le 25/08/2026, contre « 100% » auparavant. Le
# plafond n'arrive qu'une fois par fenêtre ; le reste du temps, ce quatrième
# caractère était un blanc, et il se cumulait avec l'espace de séparation en un
# trou de deux colonnes sur chacun des trois compteurs — « ctx  34% ».
#
# Le prix est un débordement d'un caractère à 100 %, exactement celui déjà
# accepté pour le « ~100% » d'une valeur mémorisée.
$LargeurValeur = 3

# Crans de la micro-jauge, du plus vide au plus plein — deux cellules depuis le
# 26/08/2026, second lot de retouches.
#
# Un nombre se lit, un remplissage se voit. La jauge donne l'état d'une fenêtre
# avant que l'œil n'atteigne le chiffre, pour deux colonnes.
#
# Sept crans, et non huit. La cellule de gauche se remplit d'abord, celle de
# droite ensuite : c'est la sémantique d'une jauge, et elle se lit sans qu'on ait
# à comparer deux densités entre elles. Avec quatre densités par cellule, la
# chaîne strictement croissante que cela autorise compte sept crans et pas un de
# plus — ░░ ▒░ ▓░ █░ █▒ █▓ ██ — puisqu'un cran comme ▒▒ ne se range ni avant ni
# après █░ sans arbitraire. Un cran vaut donc 14,3 points, contre 25 avec une
# cellule unique.
#
# La jauge était en hauteur — ▁▂▃▄▅▆▇█ — jusqu'au 25/08/2026, sur la foi d'un
# commentaire affirmant que Consolas possédait le bloc « Block Elements » en
# entier. La table de la police dit le contraire : ▄ et █ y sont, les six autres
# n'y sont pas. Le terminal les faisait donc dessiner par une police de repli,
# qui ignore le corps 8 light demandé et son propre alignement vertical : la
# jauge changeait de main au milieu de sa propre échelle, entre ▃ et ▄.
#
# D'où la densité : ░▒▓█ sont tous les quatre dans Consolas, et la densité dit
# « plein » plus directement que la hauteur, qui se compare mal d'un caractère
# isolé à un autre. Le nombre de crans n'est écrit nulle part ailleurs :
# Get-BlocJauge découpe les cent points sur la longueur de cette table, quelle
# que soit la largeur des crans. Seize crans resteraient hors d'atteinte sans
# changer de police, les blocs partiels ▏▎▍▌▋▊▉ étant absents de Consolas comme
# les blocs de hauteur.
#
# La jauge est réservée aux fenêtres de limitation et ne paraît pas sur le
# contexte : une fenêtre se remplit vers un plafond qu'on subit, alors que le
# contexte se compacte — lui donner une jauge suggérerait une fatalité qu'il
# n'a pas.
#
# Les densités sont nommées, et écrites par code point comme le reste des
# glyphes du fichier : la table se lit alors comme un remplissage plutôt que
# comme une suite de codes.
$JaugeVide = [char]0x2591
$JaugeLeger = [char]0x2592
$JaugeMoyen = [char]0x2593
$JaugePlein = [char]0x2588
$BlocsJauge = @(
    "$JaugeVide$JaugeVide",
    "$JaugeLeger$JaugeVide",
    "$JaugeMoyen$JaugeVide",
    "$JaugePlein$JaugeVide",
    "$JaugePlein$JaugeLeger",
    "$JaugePlein$JaugeMoyen",
    "$JaugePlein$JaugePlein"
)

# Nombre de segments de chemin affichés avant repli en « …\feuille » —
# « racine\…\feuille » jusqu'au 23/09/2026. Deux, et non plus trois, depuis le
# même jour : « PY_xl\.claude\user-config » est déjà un chemin long.
$ProfondeurMaxChemin = 2

# Remontée maximale à la recherche d'un « .git ». Un lien mal formé ne peut
# ainsi pas faire tourner la boucle indéfiniment, et 32 niveaux dépassent
# toute arborescence réelle.
$RemonteeMaxGit = 32

# Fichiers volatiles, sous %LOCALAPPDATA% et non à côté du script : la copie
# versionnée est sur un lecteur Google Drive, où un fichier réécrit à chaque
# changement de pourcentage n'a rien à faire.
$DossierEtat = "claude-code"
$NomCache = "statusline-cache.json"
$NomJournal = "statusline-debug.log"

# ===========================================================================
# 2. Sortie
# ===========================================================================

# Écrit la ligne sur la sortie standard, en octets UTF-8, sans passer par le
# formatage de PowerShell.
#
# Claude Code lance ce script sur un tube, sans console attachée. Dans ces
# conditions, deux couches de PowerShell abîment la ligne :
#
#   - l'encodage : [Console]::OutputEncoding retombe sur la page de codes OEM
#     (850 ici), où « · » tient en un octet 0xFA. Ce seul octet est invalide en
#     UTF-8, et le lecteur affiche « � » à la place du séparateur ;
#   - la couleur : $PSStyle.OutputRendering vaut « Host » par défaut, ce qui
#     retire les séquences ANSI dès que la sortie n'est pas un terminal.
#
# Écrire les octets soi-même court-circuite les deux, et vaut aussi pour
# Windows PowerShell 5.1, dépourvu de $PSStyle.
#
# Le crabe clawd était posé ici, et nulle part ailleurs, du 26/08/2026 au soir du
# même jour : cette fonction est le seul passage que tous les chemins traversent,
# Write-Repli compris, si bien que la marque paraissait jusque sur une ligne
# réduite au nom du modèle. L'abonnement a repris sa pastille mais pas cette
# place — il est un segment, il s'assemble avec les autres et se retire comme
# eux. Voir Get-SegmentAbonnement.
function Write-LigneStatut {
    param([string]$Ligne)

    $octets = [System.Text.Encoding]::UTF8.GetBytes($Ligne + "`n")
    $sortie = [Console]::OpenStandardOutput()
    $sortie.Write($octets, 0, $octets.Length)
    $sortie.Flush()
}

# Écrit un repli sans jamais lever : dernier recours des chemins d'erreur.
#
# Si l'écriture elle-même échoue, il n'y a plus rien à tenter et le silence
# est la seule issue.
function Write-Repli {
    param([string]$Ligne)

    try {
        Write-LigneStatut $Ligne
    }
    catch {
        return
    }
}

# Rend la ligne telle quelle si elle porte quelque chose, sinon un repli.
#
# Une ligne blanche n'est pas un affichage minimal, c'est un effacement :
# Claude Code découpe la sortie du script, retire les lignes vides, et un
# résultat vide remplace le texte affiché par rien. Tous les segments pouvant
# se retirer d'eux-mêmes, la ligne assemblée doit passer par ce filtre — et le
# repli lui-même, qui vient du payload, est vérifié à son tour.
function Confirm-Ligne {
    param(
        [string]$Ligne,
        [string]$Repli
    )

    if (-not [string]::IsNullOrWhiteSpace($Ligne)) {
        return $Ligne
    }

    if (-not [string]::IsNullOrWhiteSpace($Repli)) {
        return $Repli
    }

    return $script:ModeleParDefaut
}

# Enveloppe un fragment dans une séquence ANSI de couleur.
#
# La coloration ne porte que sur le fragment concerné, jamais sur la ligne
# entière : le reste de la ligne doit conserver le style du thème. La
# convention NO_COLOR (no-color.org) est respectée, ce qui rend aussi la
# sortie testable sans avoir à filtrer les séquences d'échappement.
function Add-Couleur {
    param(
        [string]$Texte,
        [string]$Code
    )

    if ($env:NO_COLOR) {
        return $Texte
    }

    # Concaténation plutôt qu'interpolation : dans une chaîne double, PowerShell
    # interpréterait « $echap[0m » comme une indexation de variable.
    $echap = [char]27
    return $echap + "[" + $Code + "m" + $Texte + $echap + "[0m"
}

# Atténue un fragment de chrome : libellé, unité, ponctuation porteuse d'aucune
# valeur. Passe par Add-Couleur, et hérite donc du respect de NO_COLOR.
function Add-Attenuation {
    param([string]$Texte)

    return (Add-Couleur $Texte $script:CodeAttenue)
}

# Pose un marqueur de session : contexte étendu, mode rapide, réflexion coupée.
#
# Ces trois-là ne sont ni du chrome ni une mesure. Ils ne paraissent que sur
# l'état inhabituel, et leur teinte propre les détache du nom du modèle auquel
# ils étaient jusqu'ici indistinguables. Voir $CodeMarqueur.
function Add-Marqueur {
    param([string]$Texte)

    return (Add-Couleur $Texte $script:CodeMarqueur)
}

# Rend le séparateur de segment, prêt à être posé entre deux segments d'un même
# groupe.
#
# Un séparateur structure, il n'informe pas, et n'a donc rien à faire dans la
# couleur pleine du thème. Voir $CodeSeparateur.
#
# C'est le seul des trois rangs à passer par une fonction, parce que c'est le
# seul à s'écrire par un glyphe : le rang supérieur est un espace depuis que la
# pastille borne son groupe — voir $SeparateurGroupe —, et un blanc reste hors
# des séquences de couleur, comme partout ailleurs sur la ligne.
function Get-Separateur {
    return (Add-Couleur $script:Separateur $script:CodeSeparateur)
}

# Pose un fragment en pastille : fond vert profond, texte vert clair, le fond
# épousant exactement le texte.
#
# Réservée à l'emplacement ; le raisonnement qui l'y cantonne est en tête de
# $CodePastille. L'abonnement porte la seconde des deux pastilles de la ligne,
# dans sa teinte propre — voir Add-PastilleAbonnement.
#
# La pastille a porté quelques heures un espace de part et d'autre à l'intérieur
# du fond, pour que le vert ne colle pas aux caractères. Il coûtait deux colonnes
# par pastille, quatre sur la ligne entière, et sur une grille de terminal il n'y
# a pas de cran intermédiaire : un liseré fait une cellule pleine ou rien. La
# compacité l'a emporté sur l'aération le 26/08/2026 au soir.
#
# Ce qui reste tenable parce que le fond EST la mise en valeur : il porte la
# limite du segment à lui seul, et un liseré n'y ajoutait qu'un confort. La
# fonction se réduit dès lors à Add-Couleur sous un autre nom, et c'est
# délibéré — le nom dit l'intention, que $CodePastille seul ne dirait pas.
#
# Le même argument a emporté la barre de groupe quelques heures plus tard : le
# fond ne borne pas seulement le segment, il le détache aussi de ses voisins, et
# deux barres l'encadraient pour redire cela. La pastille porte donc désormais
# les deux rôles — voir $SeparateurGroupe.
function Add-Pastille {
    param([string]$Texte)

    return (Add-Couleur $Texte $script:CodePastille)
}

# Pose un fragment en pastille de tête : la teinte de marque du produit, fond
# profond et texte clair, le fond épousant exactement le texte.
#
# Même forme qu'Add-Pastille, autre teinte — voir $CodePastilleAbonnement. Deux
# fonctions plutôt qu'un paramètre : ce qui distingue les deux pastilles n'est
# pas une couleur passée à un appelant, mais ce qu'elles marquent, et chacune n'a
# qu'un seul appelant possible.
#
# Sous NO_COLOR, le fragment garde son texte, comme celui d'Add-Pastille et
# contrairement au logo qui occupait cette place jusqu'au 26/08/2026 au soir : le
# crabe était une marque que la couleur constituait, un abonnement est une
# information que la couleur décore.
function Add-PastilleAbonnement {
    param([string]$Texte)

    return (Add-Couleur $Texte $script:CodePastilleAbonnement)
}

# Colore un segment selon le palier atteint : critique, alerte, ou aucun.
#
# Les deux paliers sont testés du plus haut au plus bas, si bien qu'un
# $SeuilCritique mal réglé — inférieur à $Seuil — dégrade l'affichage vers le
# rouge plutôt que de rendre l'un des deux paliers inatteignable.
#
# $SeuilCritique omis vaut « jamais » plutôt que zéro : sans cette valeur par
# défaut, un appelant qui l'ignore peindrait en rouge jusqu'au 0 %.
function Add-Alerte {
    param(
        [string]$Segment,
        [int]$Pourcent,
        [int]$Seuil,
        [int]$SeuilCritique = [int]::MaxValue
    )

    if ($Pourcent -ge $SeuilCritique) {
        return (Add-Couleur $Segment $script:CodeCritique)
    }

    if ($Pourcent -ge $Seuil) {
        return (Add-Couleur $Segment $script:CodeAlerte)
    }

    return $Segment
}

# ===========================================================================
# 3. Conversions défensives
# ===========================================================================

# Convertit un pourcentage du payload en entier arrondi, ramené entre 0 et
# 100, ou $null si la valeur n'est pas exploitable.
#
# Le contrat annonce un nombre, mais une conversion directe « [double]$valeur »
# lève sur une chaîne non numérique et emporte toute la ligne. La culture
# invariante est imposée : le séparateur décimal du payload est le point, alors
# que la culture du poste est française.
#
# Ramené dans les bornes depuis le 04/09/2026, comme convertir_pourcent côté
# Rust : un payload à 150 affichait « 150% » et projetait une heure
# d'épuisement dans le passé, un payload à -5 affichait « -5% ». Le cast [int]
# reste avant la borne, pour qu'une valeur hors de sa plage soit un rejet et
# non un plafond.
function ConvertTo-Pourcent {
    param($Valeur)

    $nombre = ConvertTo-Nombre $Valeur
    if ($null -eq $nombre) {
        return $null
    }

    return [math]::Clamp([int][math]::Round($nombre), 0, 100)
}

# Convertit une valeur quelconque en double, ou $null si elle n'a pas de sens
# numérique. Les valeurs non finies sont écartées : elles ne se formatent pas.
function ConvertTo-Nombre {
    param($Valeur)

    if ($null -eq $Valeur) {
        return $null
    }

    try {
        $nombre = [System.Convert]::ToDouble(
            $Valeur, [System.Globalization.CultureInfo]::InvariantCulture)
    }
    catch {
        return $null
    }

    if ([double]::IsNaN($nombre) -or [double]::IsInfinity($nombre)) {
        return $null
    }

    return $nombre
}

# Convertit un horodatage du payload en DateTimeOffset, ou $null si la valeur
# n'est pas exploitable.
#
# « resets_at » est documenté en secondes Unix, et c'est bien ce qu'envoie
# Claude Code 2.1.220. Trois autres écritures sont acceptées sans rien coûter,
# parce qu'un format inattendu faisait auparavant disparaître la ligne entière
# plutôt que le seul segment : le [datetime] que ConvertFrom-Json fabrique
# lui-même à partir d'une chaîne datée, l'ISO 8601 resté sous forme de chaîne,
# et les millisecondes.
function ConvertTo-Instant {
    param($Valeur)

    if ($null -eq $Valeur) {
        return $null
    }

    if ($Valeur -is [System.DateTimeOffset]) {
        return $Valeur
    }

    if ($Valeur -is [datetime]) {
        try {
            return [System.DateTimeOffset]::new($Valeur)
        }
        catch {
            return $null
        }
    }

    # La chaîne datée passe avant la voie numérique : « 20260802 » se lirait
    # sinon comme un compte de secondes, soit une date de 1970.
    if ($Valeur -is [string]) {
        $instant = [System.DateTimeOffset]::MinValue
        if ([System.DateTimeOffset]::TryParse(
                $Valeur,
                [System.Globalization.CultureInfo]::InvariantCulture,
                [System.Globalization.DateTimeStyles]::AssumeUniversal,
                [ref]$instant)) {
            return $instant
        }
    }

    $nombre = ConvertTo-Nombre $Valeur
    if ($null -eq $nombre) {
        return $null
    }

    # 1e11 secondes tombent en l'an 5138 : au-delà, l'unité est la milliseconde.
    $secondes = [long]$nombre
    if ([math]::Abs($secondes) -ge 100000000000L) {
        $secondes = [long]($secondes / 1000)
    }

    try {
        return [System.DateTimeOffset]::FromUnixTimeSeconds($secondes)
    }
    catch {
        # Hors de la plage représentable, malgré le repli sur les millisecondes.
        return $null
    }
}

# ===========================================================================
# 4. État sur disque : cache et journal
# ===========================================================================

# Chemin d'un fichier d'état, ou $null si aucune racine n'est disponible.
function Get-CheminEtat {
    param([string]$Nom)

    $racine = if ($env:LOCALAPPDATA) { $env:LOCALAPPDATA } else { $env:TEMP }
    if (-not $racine) {
        return $null
    }

    return (Join-Path $racine (Join-Path $script:DossierEtat $Nom))
}

# Crée le dossier parent d'un fichier s'il manque.
function Confirm-DossierParent {
    param([string]$Chemin)

    $dossier = Split-Path -Parent $Chemin
    if ($dossier -and -not (Test-Path -LiteralPath $dossier)) {
        New-Item -ItemType Directory -Path $dossier -Force | Out-Null
    }
}

# Relit le cache des fenêtres, ou $null s'il est absent ou illisible.
#
# Le cache existe parce que Claude Code ne renseigne « rate_limits » qu'à
# partir de la première requête de la session : sans lui, la ligne se réduirait
# au modèle tant que rien n'a été envoyé.
#
# Il porte une entrée par fenêtre, sous la clé du payload :
#
#   { "five_hour": { "used_percentage": 29, "resets_at": 1786303800,
#                    "observe_a": 1786288000, "observe_pourcent": 12,
#                    "observe_modele": null },
#     "seven_day": { …, "observe_modele": "fable" } }
#
# Les deux premiers champs servent l'affichage anticipé, les deux suivants le
# calcul du rythme : ils gardent la toute première mesure vue pour cette
# fenêtre, ce qui donne une pente réellement observée plutôt que déduite d'une
# durée de fenêtre supposée. Voir Get-Ancrage et Format-Rythme. Le dernier,
# ajouté le 03/09/2026, est la famille du modèle sous laquelle cet ancrage a été
# posé — pour les seules fenêtres qui suivent le modèle, null ailleurs : voir
# $FacteursModele.
#
# Un cache écrit par une version antérieure du script — plat, sans les deux
# champs d'ancrage, ou sans la famille — est relu sans erreur : les champs
# manquants valent $null, et seul le segment qui en dépend se retire, ou se
# contente de moins. C'est sans conséquence, le cache étant purement volatile.
function Get-CacheFenetres {
    try {
        $chemin = Get-CheminEtat $script:NomCache
        if (-not $chemin -or -not (Test-Path -LiteralPath $chemin)) {
            return $null
        }

        return (Get-Content -LiteralPath $chemin -Raw -Encoding UTF8 |
            ConvertFrom-Json)
    }
    catch {
        return $null
    }
}

# Extrait d'un cache relu l'entrée d'une fenêtre, ou $null si elle est absente,
# incomplète ou périmée.
#
# Une fenêtre dont l'heure de remise à zéro est passée est jetée : le compteur
# est reparti de zéro entre-temps, et réafficher l'ancien pourcentage serait
# faux. Mieux vaut alors le segment absent, le temps de la première requête.
function Get-FenetreMemorisee {
    param(
        $Cache,
        [string]$Cle
    )

    if ($null -eq $Cache) {
        return $null
    }

    try {
        $entree = $Cache.$Cle
        if ($null -eq $entree) {
            return $null
        }
        if ($null -eq $entree.used_percentage -or $null -eq $entree.resets_at) {
            return $null
        }

        $reset = ConvertTo-Instant $entree.resets_at
        if ($null -eq $reset -or $reset -le [System.DateTimeOffset]::UtcNow) {
            return $null
        }

        return $entree
    }
    catch {
        # Cache dégénéré — un tableau, un scalaire — où l'accès par clé n'a pas
        # de sens.
        return $null
    }
}

# Mémorise les fenêtres courantes, sans réécrire un contenu identique.
#
# Les deux fenêtres sont écrites en un seul passage, et $Entrees porte donc
# aussi celles qui ne viennent que du cache : le payload peut n'en renseigner
# qu'une, et une écriture partielle perdrait l'autre. Une fenêtre dont le
# pourcentage n'est pas exploitable est laissée de côté sans empêcher l'autre
# d'être écrite.
#
# L'écriture passe par un fichier temporaire renommé : plusieurs sessions
# Claude Code partagent ce cache, et un déplacement ne laisse jamais lire un
# fichier à moitié écrit. Toute erreur est absorbée : un cache qu'on n'a pas pu
# écrire ne doit pas coûter la ligne de statut.
function Save-Fenetres {
    param($Entrees)

    try {
        $chemin = Get-CheminEtat $script:NomCache
        if (-not $chemin) {
            return
        }

        # Le cache est normalisé — pourcentage entier, secondes Unix, famille
        # en chaîne non blanche ou null — quel que soit le format reçu, pour que
        # Get-FenetreMemorisee n'ait qu'une seule écriture à relire. Les champs
        # absents sont écrits à null plutôt qu'omis : la forme du fichier reste
        # la même d'une écriture à l'autre.
        $normalise = [ordered]@{}
        foreach ($cle in $Entrees.Keys) {
            $entree = $Entrees[$cle]

            $pourcent = ConvertTo-Pourcent $entree.used_percentage
            if ($null -eq $pourcent) {
                continue
            }

            $instant = ConvertTo-Instant $entree.resets_at
            $ancre = ConvertTo-Nombre $entree.observe_a
            $modele = $entree.observe_modele

            $normalise[$cle] = [ordered]@{
                used_percentage  = $pourcent
                resets_at        = if ($null -ne $instant) {
                    $instant.ToUnixTimeSeconds()
                } else {
                    $null
                }
                observe_a        = if ($null -ne $ancre) { [long]$ancre } else { $null }
                observe_pourcent = ConvertTo-Pourcent $entree.observe_pourcent
                observe_modele   = if ($modele -is [string] -and $modele.Trim()) {
                    $modele
                } else {
                    $null
                }
            }
        }

        if ($normalise.Count -eq 0) {
            return
        }

        $contenu = $normalise | ConvertTo-Json -Depth 4 -Compress

        if ((Test-Path -LiteralPath $chemin) -and
            ((Get-Content -LiteralPath $chemin -Raw -Encoding UTF8) -eq $contenu)) {
            return
        }

        Confirm-DossierParent $chemin

        $temporaire = "$chemin.$PID.tmp"
        Set-Content -LiteralPath $temporaire -Value $contenu -Encoding UTF8 -NoNewline
        Move-Item -LiteralPath $temporaire -Destination $chemin -Force
    }
    catch {
        return
    }
}

# Journalise une exception et le payload qui l'a provoquée, si
# CLAUDE_STATUSLINE_DEBUG est défini.
#
# Le diagnostic passe par un fichier, jamais par la sortie standard : celle-ci
# est la ligne affichée, pas un canal de trace. Le journal reste hors du
# projet : il porte les chemins de travail de la session.
function Write-Diagnostic {
    param(
        $Erreur,
        [string]$Payload
    )

    if (-not $env:CLAUDE_STATUSLINE_DEBUG) {
        return
    }

    try {
        $chemin = Get-CheminEtat $script:NomJournal
        if (-not $chemin) {
            return
        }

        Confirm-DossierParent $chemin

        $horodatage = (Get-Date).ToString(
            "yyyy-MM-dd HH:mm:ss",
            [System.Globalization.CultureInfo]::InvariantCulture)
        $lignes = @(
            "[$horodatage] $($Erreur.Exception.GetType().Name) : " +
                "$($Erreur.Exception.Message)"
            "  a : $($Erreur.InvocationInfo.PositionMessage -replace '\r?\n', ' ')"
            "  payload : $Payload"
        )

        Add-Content -LiteralPath $chemin -Value $lignes -Encoding UTF8
    }
    catch {
        # Un journal de diagnostic ne doit jamais coûter la ligne de statut.
        return
    }
}

# ===========================================================================
# 5. Lectures sur disque : dépôt Git, abonnement
# ===========================================================================
#
# Deux informations manquent au contrat d'entrée et se lisent donc sur le
# disque — trois jusqu'au retrait du segment de version, le 23/09/2026. Dans
# les deux cas la même contrainte gouverne : la ligne se
# rafraîchit souvent, et lancer un processus à chaque fois se paierait cher —
# le workspace est sur un lecteur réseau. Tout passe par des lectures de
# fichier.
#
# La branche d'abord : « worktree.branch » n'est renseigné que pour les
# sessions lancées avec --worktree. Elle est donc lue directement dans
# « .git/HEAD », plutôt qu'en appelant « git », ce qui aurait en outre supposé
# git présent dans le PATH.

# Remonte les dossiers parents jusqu'au premier « .git », et rend son chemin,
# ou $null hors dépôt.
function Find-CheminGit {
    param([string]$Depart)

    $courant = [System.IO.DirectoryInfo]$Depart

    for ($i = 0; $i -lt $script:RemonteeMaxGit -and $null -ne $courant; $i++) {
        $candidat = Join-Path $courant.FullName ".git"
        if (Test-Path -LiteralPath $candidat) {
            return $candidat
        }
        $courant = $courant.Parent
    }

    return $null
}

# Rend le dossier « .git » réel derrière un chemin trouvé, ou $null.
#
# Les deux formes sont gérées : le dossier habituel, et le fichier
# « gitdir: <chemin> » que posent les worktrees liés et les sous-modules.
function Resolve-DossierGit {
    param([string]$Chemin)

    if (Test-Path -LiteralPath $Chemin -PathType Container) {
        return $Chemin
    }

    $pointeur = (Get-Content -LiteralPath $Chemin -Raw -Encoding UTF8).Trim()
    if ($pointeur -notmatch '(?m)^gitdir:\s*(.+)$') {
        return $null
    }

    $cible = $Matches[1].Trim()
    if (-not [System.IO.Path]::IsPathRooted($cible)) {
        $cible = Join-Path (Split-Path -Parent $Chemin) $cible
    }

    return $cible
}

# Lit le nom de branche dans un dossier « .git », ou $null.
function Read-BrancheHead {
    param([string]$DossierGit)

    $head = Join-Path $DossierGit "HEAD"
    if (-not (Test-Path -LiteralPath $head)) {
        return $null
    }

    $contenu = (Get-Content -LiteralPath $head -Raw -Encoding UTF8).Trim()
    if ($contenu -match '^ref:\s*refs/heads/(.+)$') {
        return $Matches[1].Trim()
    }

    # HEAD détachée : le SHA abrégé situe mieux qu'un segment absent.
    if ($contenu -match '^[0-9a-fA-F]{7,40}$') {
        return $contenu.Substring(0, 7)
    }

    return $null
}

# Renvoie la branche Git couvrant un répertoire, ou $null hors dépôt.
#
# Seule frontière protégée des trois étapes ci-dessus : un dépôt biscornu ou
# un fichier illisible retire le segment, sans jamais coûter la ligne.
function Get-BrancheGit {
    param([string]$Depart)

    try {
        if ([string]::IsNullOrWhiteSpace($Depart) -or
            -not (Test-Path -LiteralPath $Depart)) {
            return $null
        }

        $chemin = Find-CheminGit $Depart
        if (-not $chemin) {
            return $null
        }

        $dossier = Resolve-DossierGit $chemin
        if (-not $dossier) {
            return $null
        }

        return (Read-BrancheHead $dossier)
    }
    catch {
        return $null
    }
}

# Rend le chemin de la configuration Claude Code, ou $null s'il est introuvable.
#
# CLAUDE_STATUSLINE_CONFIG l'emporte, et sans repli — convention reprise de
# CLAUDE_STATUSLINE_BINAIRE, disparu avec le segment de version le 23/09/2026 :
# une désignation explicite qui ne résout pas doit se voir, pas se faire
# remplacer en silence par l'emplacement habituel. Le harnais de non-régression s'en sert pour couvrir
# les replis sans rien supposer du compte réel du poste.
function Find-Config {
    if ($env:CLAUDE_STATUSLINE_CONFIG) {
        if (Test-Path -LiteralPath $env:CLAUDE_STATUSLINE_CONFIG -PathType Leaf) {
            return $env:CLAUDE_STATUSLINE_CONFIG
        }
        return $null
    }

    if (-not $env:USERPROFILE) {
        return $null
    }

    $natif = Join-Path $env:USERPROFILE $script:CheminConfigRelatif
    if (Test-Path -LiteralPath $natif -PathType Leaf) {
        return $natif
    }

    return $null
}

# Rend lisible un identifiant en snake_case dont la table ne sait rien.
#
# « claude_pro » devient « Pro ». C'est exactement ce que la table produit pour
# les valeurs connues, et c'est voulu : le jour où Claude Code ajoute un type
# d'abonnement, la ligne l'affiche correctement sans qu'on ait rien à changer. La
# table reste utile pour les cas où cette mécanique ne suffirait pas — un palier
# à joindre, une casse à respecter.
#
# Le « claude » de tête est élagué depuis le 26/08/2026, comme il l'est dans
# $Abonnements : sans cela la table dirait « Pro » et le repli « Claude Ultra
# Plus », et l'égalité que ce commentaire revendique entre les deux voies
# cesserait d'être vraie. L'élagage porte sur le premier morceau seulement, et
# jamais sur le dernier — un « claude » nu resterait « Claude » plutôt que de
# disparaître, un libellé vide retirant le segment au lieu de le raccourcir.
#
# Les morceaux vides sont écartés, ce qui rend une chaîne de séparateurs seuls à
# $null du côté de Format-Abonnement plutôt qu'à un libellé blanc.
function ConvertTo-LibelleLisible {
    param([string]$Identifiant)

    # @() force le tableau : un identifiant sans « _ » ne rendrait sinon qu'une
    # chaîne, dont « .Count » vaut 1 mais dont l'indexation donne un caractère.
    $morceaux = @($Identifiant -split '_' | Where-Object { $_ })

    # Les morceaux vides sont déjà partis : plus d'un morceau suffit donc à
    # garantir qu'élaguer ne laissera pas le libellé sans rien.
    if ($morceaux.Count -gt 1 -and $morceaux[0].ToLowerInvariant() -eq "claude") {
        $morceaux = $morceaux[1..($morceaux.Count - 1)]
    }

    return (($morceaux | ForEach-Object {
        $_.Substring(0, 1).ToUpperInvariant() + $_.Substring(1).ToLowerInvariant()
    }) -join " ")
}

# Compose le libellé affiché à partir du type d'organisation et du palier de
# limitation, ou $null si le type ne dit rien.
#
# Les comparaisons sont sensibles à la casse (« -ceq ») : la table décrit des
# identifiants du contrat, pas des mots. Une valeur qui n'en respecte pas la
# casse passe donc par le repli, où elle est normalisée — et le résultat est le
# même, ce qui est bien la garantie recherchée.
#
# Le palier n'est joint qu'à un abonnement Max, seul cas où Claude Code lui-même
# le fait. Un palier inconnu, ou celui d'un compte Pro (« default_claude_ai »),
# laisse le libellé nu : « Claude Max » sans chiffre reste juste, là où un
# suffixe inventé ne le serait pas.
function Format-Abonnement {
    param(
        [string]$TypeOrganisation,
        [string]$Palier
    )

    $typeNet = $TypeOrganisation.Trim()
    if ([string]::IsNullOrEmpty($typeNet)) {
        return $null
    }

    $connu = $script:Abonnements | Where-Object { $_.Cle -ceq $typeNet } | Select-Object -First 1
    $base = if ($connu) { $connu.Libelle } else { ConvertTo-LibelleLisible $typeNet }

    if ([string]::IsNullOrEmpty($base)) {
        return $null
    }

    if ($typeNet -cne $script:TypeAbonnementMax -or $null -eq $Palier) {
        return $base
    }

    $palierNet = $Palier.Trim()
    $cran = $script:PaliersMax | Where-Object { $_.Cle -ceq $palierNet } | Select-Object -First 1
    if (-not $cran) {
        return $base
    }

    return "$base $($cran.Suffixe)"
}

# Lit l'abonnement du compte connecté, ou $null s'il n'est pas lisible.
#
# Le payload ne porte rien de l'abonnement : les trente-quatre champs relevés le
# 26/08/2026 sur la version 2.1.246 sont muets là-dessus. La valeur vit donc là
# où Claude Code l'écrit lui-même, « ~/.claude.json », sous
# « oauthAccount.organizationType ».
#
# Deux fichiers portent la réponse, et le choix entre eux n'est pas indifférent :
# « ~/.claude/.credentials.json » est vingt fois plus court, et c'est l'autre qui
# est lu. Une ligne de statut n'a aucune raison d'ouvrir le fichier qui porte le
# jeton d'accès et son jeton de rafraîchissement — un programme qui ne l'ouvre
# jamais ne peut pas le divulguer. Le workspace applique déjà cette règle à sa
# sauvegarde, qui refuse de traiter « .credentials.json ».
#
# Rien n'y est fatal : fichier absent, JSON illisible, champ disparu d'une
# version à l'autre — chacun de ces chemins retire le segment plutôt que
# d'inventer une valeur.
#
# La lecture du fichier est séparée depuis le soir du 03/09/2026 : deux
# segments s'y servent — l'abonnement ici, et la fenêtre propre au modèle dans
# Get-FenetreModele — et le fichier fait 64 Ko. Voir Get-Configuration.
function Get-Abonnement {
    param($Config)

    try {
        if ($null -eq $Config) {
            return $null
        }

        $compte = $Config.oauthAccount
        if ($null -eq $compte) {
            return $null
        }

        $type = $compte.organizationType
        if ($null -eq $type -or $type -isnot [string]) {
            return $null
        }

        $palier = $compte.organizationRateLimitTier
        if ($palier -isnot [string]) {
            $palier = $null
        }

        return (Format-Abonnement -TypeOrganisation $type -Palier $palier)
    }
    catch {
        return $null
    }
}

# Lit et analyse la configuration Claude Code, ou $null si elle n'est pas
# lisible — fichier absent, JSON tronqué par une réécriture en cours. Lue une
# fois par lancement, pour les deux segments qui s'y servent.
function Get-Configuration {
    try {
        $chemin = Find-Config
        if (-not $chemin) {
            return $null
        }

        return (Get-Content -LiteralPath $chemin -Raw -Encoding UTF8 | ConvertFrom-Json)
    }
    catch {
        return $null
    }
}

# Rend la fenêtre hebdomadaire propre à la famille du modèle, relevée par
# /usage, ou $null — 03/09/2026, au soir.
#
# Le contrat d'entrée ne porte que deux fenêtres, « five_hour » et
# « seven_day », lues dans les en-têtes de la dernière réponse API. L'écran
# /usage affiche une fenêtre de plus pour les modèles qui en ont une — « Current
# week (Fable) » —, et c'est elle qui décide du passage de Fable aux crédits
# payants. Elle vient d'un appel à /api/oauth/usage, dont Claude Code persiste
# la réponse dans ~/.claude.json sous cachedUsageUtilization : un fetchedAtMs,
# l'accountUuid du compte, et sous utilization.limits[] une entrée par barre de
# l'écran — « session », « weekly_all », et une « weekly_scoped » par modèle,
# portant scope.model.display_name.
#
# Fraîcheur, relevée dans le bundle de claude.exe 2.1.259 : la réponse n'est
# réécrite qu'au plus toutes les cinq minutes, à l'occasion d'un appel —
# l'ouverture de /usage, pour l'essentiel —, et Claude Code lui-même ne relit
# la valeur persistée que si elle a moins d'une heure. Voir $AgeMaxUsagePersiste
# pour ce que la ligne en fait.
#
# $null couvre tout ce qui ne permet pas d'afficher une valeur honnête :
# configuration absente ou muette, relevé d'un autre compte que celui connecté
# — Claude Code fait la même vérification —, relevé daté d'un instant qui n'est
# pas encore venu, entrée dont le pourcentage ne se lit pas, remise à zéro
# illisible ou déjà passée, et bien sûr famille inconnue ou sans fenêtre
# propre. La première entrée « weekly_scoped » dont le modèle porte le nom de
# la famille l'emporte, la casse ignorée.
#
# Une remise à zéro NULLE n'est pas une remise à zéro illisible — 04/09/2026.
# Tant que le modèle n'a rien consommé depuis la dernière, l'API rend pour sa
# fenêtre percent 0 et resets_at null : la fenêtre n'a pas commencé, et n'a
# donc pas d'échéance. /usage l'affiche quand même — son composant, relevé dans
# le bundle de claude.exe 2.1.259, ne se retire que sur un utilization nul et
# omet seulement la ligne « Resets » — et la ligne fait de même : la fenêtre
# passe avec son pourcentage et sans échéance, ce qui lui retire le rythme et
# l'heure, jamais le pourcentage. Sans cela, la ligne retombait sur la fenêtre
# globale chaque jeudi soir, au moment précis où /usage disait « Current week
# (Fable) 0 % ».
#
# Les deux valeurs rendues sont brutes — « percent » et « resets_at » tels
# qu'écrits — pour passer par les mêmes conversions qu'une fenêtre du payload ;
# releve_a est l'instant du relevé en secondes Unix, et c'est lui qui date la
# mesure. ConvertFrom-Json a déjà converti la date ISO en [datetime] local, ce
# que ConvertTo-Instant sait relire à la seconde près.
function Get-FenetreModele {
    param(
        $Config,
        [string]$Famille,
        [long]$Maintenant
    )

    if ($null -eq $Config -or -not $Famille) {
        return $null
    }

    try {
        $usage = $Config.($script:CleUsagePersiste)
        if ($null -eq $usage) {
            return $null
        }

        # Même compte que celui connecté, quand les deux identifiants existent :
        # un relevé sans compte n'est pas un désaccord mais une inconnue.
        $connecte = $Config.oauthAccount.accountUuid
        $releve = $usage.accountUuid
        if ($connecte -is [string] -and $releve -is [string] -and $connecte -cne $releve) {
            return $null
        }

        $releveA = ConvertTo-Instant $usage.fetchedAtMs
        if ($null -eq $releveA) {
            return $null
        }
        $releveA = $releveA.ToUnixTimeSeconds()
        if ($releveA -gt $Maintenant) {
            return $null
        }

        $limites = $usage.utilization.limits
        if ($limites -isnot [array]) {
            return $null
        }

        # La fenêtre de tous les modèles, vue par le même relevé : le point de
        # référence de la dérive — 05/09/2026, voir Get-DeriveModele.
        # Pourcentage et remise à zéro tous deux exigés, la seconde servant à
        # reconnaître la même fenêtre dans le payload ; la première entrée
        # lisible l'emporte. Sans elle, la fenêtre du modèle passe quand même,
        # et s'affiche telle que relevée.
        $reference = $null
        foreach ($limite in $limites) {
            $genre = $limite.kind
            if ($genre -isnot [string] -or $genre -cne $script:GenreFenetreGlobale) {
                continue
            }
            $pourcentGlobale = ConvertTo-Pourcent $limite.percent
            if ($null -eq $pourcentGlobale) {
                continue
            }
            $resetGlobale = ConvertTo-Instant $limite.resets_at
            if ($null -eq $resetGlobale) {
                continue
            }
            $reference = [pscustomobject]@{
                pourcent = $pourcentGlobale
                reset    = $resetGlobale.ToUnixTimeSeconds()
            }
            break
        }

        foreach ($limite in $limites) {
            $genre = $limite.kind
            if ($genre -isnot [string] -or $genre -cne $script:GenreFenetreModele) {
                continue
            }

            $nom = $limite.scope.model.display_name
            if ($nom -isnot [string] -or -not $nom.ToLowerInvariant().Contains($Famille)) {
                continue
            }

            if ($null -eq (ConvertTo-Pourcent $limite.percent)) {
                continue
            }

            # Nulle, la remise à zéro est celle d'une fenêtre pas encore
            # entamée et passe telle quelle. Toute autre valeur doit se lire,
            # et désigner un instant à venir.
            if ($null -ne $limite.resets_at) {
                $reset = ConvertTo-Instant $limite.resets_at
                if ($null -eq $reset -or $reset.ToUnixTimeSeconds() -le $Maintenant) {
                    continue
                }
            }

            return [pscustomobject]@{
                used_percentage   = $limite.percent
                resets_at         = $limite.resets_at
                releve_a          = $releveA
                reference_globale = $reference
            }
        }

        return $null
    }
    catch {
        return $null
    }
}

# Estime où en est la fenêtre du modèle depuis son relevé, d'après l'avance
# prise entre-temps par la fenêtre globale du payload — ou $null quand rien
# d'honnête ne peut être estimé, et le relevé s'affiche alors tel quel.
# 05/09/2026.
#
# Le relevé ne bouge qu'à l'ouverture de /usage — revérifié sur la 2.1.261 par
# capture puis scan : un seul écrivain, sur le chemin de l'appel à
# /api/oauth/usage, que rien ne déclenche périodiquement. Mais le même relevé
# porte aussi la fenêtre globale, weekly_all, vue au même instant — et celle-là,
# le payload la donne fraîche à chaque réponse, sous rate_limits.seven_day. Ce
# que la globale a pris depuis le relevé, la fenêtre du modèle l'a pris aussi,
# au rapport des deux budgets ; et ce rapport se lit dans le relevé lui-même :
# 28 contre 14 le 03/09, 21 contre 11 le 05/09, deux fois dans les deux cas.
#
# -Globale est rate_limits.seven_day du payload. Le résultat est arrondi comme
# tout pourcentage de la ligne, et plafonné à cent. $null couvre : un relevé
# sans référence globale lisible ; une globale du payload illisible, ou dont la
# remise à zéro n'est pas celle du relevé — ce n'est plus la même fenêtre ; une
# avance nulle ou négative — un recul est une correction côté serveur, pas une
# consommation ; et un rapport qui n'existe pas, l'un des deux pourcentages
# relevés étant nul — l'état du lendemain d'une remise à zéro. La limite est
# assumée : une consommation d'un autre modèle entre deux relevés est comptée
# comme celle du modèle courant. Le cache, lui, mémorise toujours le relevé,
# jamais l'estimation.
function Get-DeriveModele {
    param(
        $Releve,
        $Globale
    )

    if ($null -eq $Releve -or $null -eq $Releve.reference_globale -or $null -eq $Globale) {
        return $null
    }

    try {
        $reference = $Releve.reference_globale
        if ($reference.pourcent -le 0) {
            return $null
        }
        $pourcentReleve = ConvertTo-Pourcent $Releve.used_percentage
        if ($null -eq $pourcentReleve -or $pourcentReleve -le 0) {
            return $null
        }

        $courant = ConvertTo-Pourcent $Globale.used_percentage
        if ($null -eq $courant) {
            return $null
        }
        $reset = ConvertTo-Instant $Globale.resets_at
        if ($null -eq $reset -or $reset.ToUnixTimeSeconds() -ne $reference.reset) {
            return $null
        }

        $avance = $courant - $reference.pourcent
        if ($avance -le 0) {
            return $null
        }

        # Même ordre d'opérations que le portage Rust, pour tomber sur le même
        # double avant l'arrondi : le rapport d'abord, puis son produit par
        # l'avance.
        $estime = $pourcentReleve + ($pourcentReleve / $reference.pourcent) * $avance
        return [math]::Clamp([int][math]::Round($estime), 0, 100)
    }
    catch {
        return $null
    }
}

# ===========================================================================
# 6. Segments de la ligne
# ===========================================================================

# Met en forme un triplet libellé/valeur/unité : libellé et unité atténués,
# valeur cadrée à droite et colorée selon le palier atteint.
#
# C'est la brique de la hiérarchie visuelle de la ligne. Tous les compteurs
# passent par elle, ce qui garantit qu'ils se lisent de la même façon : le
# libellé situe, la valeur porte l'information et se voit sans être cherchée.
#
# L'unité a rejoint le chrome le 25/08/2026. Le « % » ne varie jamais et ne porte
# aucune information ; il occupait pourtant trois colonnes en pleine couleur
# d'alerte, et le gras du palier critique s'appliquait à lui comme aux chiffres.
# C'est l'application au caractère près de la règle que la ligne tient partout
# ailleurs.
#
# Les mises en forme sont juxtaposées, jamais imbriquées : « ESC[0m » ramène tout
# à l'état neutre, si bien qu'une alerte ouverte à l'intérieur d'un libellé
# atténué refermerait les deux d'un coup. Les blancs — séparateur et calage —
# restent eux aussi hors des séquences, pour qu'aucun terminal n'ait à décider
# comment atténuer ou colorer un espace.
#
# Le calage porte sur la valeur et son unité, alors que la coloration les
# sépare : ce qui doit rester stable d'un rafraîchissement à l'autre est la
# colonne où finit le compteur, pas celle où finissent ses chiffres. Un compteur
# plus long que $LargeurValeur déborde sans être tronqué : perdre un chiffre
# coûterait plus cher que perdre l'alignement.
#
# -Jauge glisse la micro-jauge entre le libellé et la valeur : « 5h ▒░ 20% ».
# Elle prend la couleur du palier, comme la valeur qu'elle annonce, ce qui lui
# évite d'introduire un vert permanent : la ligne ne dit jamais « tout va
# bien », elle se tait quand tout va bien.
function Format-Mesure {
    param(
        [string]$Libelle,
        [string]$Valeur,
        [string]$Unite = "",
        [int]$Pourcent,
        [int]$Seuil,
        [int]$SeuilCritique = [int]::MaxValue,
        [switch]$Jauge
    )

    $calage = " " * [math]::Max(
        0, $script:LargeurValeur - ($Valeur.Length + $Unite.Length))

    $bloc = ""
    if ($Jauge) {
        $bloc = (Add-Alerte (Get-BlocJauge $Pourcent) $Pourcent $Seuil $SeuilCritique) + " "
    }

    # Une unité vide ne produit pas une séquence de couleur vide : elle
    # n'afficherait rien tout en pesant huit octets sur la ligne, et se verrait
    # sur toute comparaison octet à octet.
    $suffixe = if ($Unite) { Add-Attenuation $Unite } else { "" }

    return (Add-Attenuation $Libelle) + " " + $bloc + $calage +
        (Add-Alerte $Valeur $Pourcent $Seuil $SeuilCritique) + $suffixe
}

# Choisit le cran de jauge correspondant à un pourcentage.
#
# Les cent points sont découpés sur la longueur de $BlocsJauge, quelle qu'elle
# soit : sept crans sur deux cellules aujourd'hui, donc un cran tous les 14,3 %.
# Les valeurs hors bornes se rabattent sur les extrémités plutôt que d'indexer
# hors table.
function Get-BlocJauge {
    param([int]$Pourcent)

    $borne = [math]::Min(100, [math]::Max(0, $Pourcent))
    $index = [math]::Min($script:BlocsJauge.Count - 1,
        [int][math]::Floor($borne * $script:BlocsJauge.Count / 100))

    return [string]$script:BlocsJauge[$index]
}

# Replie une suite de segments de chemin en « …\feuille » au-delà de la
# profondeur maximale, pour ne pas manger la ligne. La racine du projet restait
# en tête jusqu'au 23/09/2026 ; seule la feuille demeure depuis, à la demande
# de l'utilisateur.
function Compress-Chemin {
    param([string[]]$Segments)

    if ($Segments.Count -le $script:ProfondeurMaxChemin) {
        return $Segments
    }

    return @("…", $Segments[-1])
}

# Met en forme le répertoire de travail, relativement au répertoire de
# lancement de la session.
#
# La feuille seule ne suffit pas sur ce workspace multi-projets : « tests » ou
# « src » ne disent pas d'où l'on parle. Le chemin est donc affiché depuis la
# racine du projet. Hors du projet, la feuille seule fait l'affaire.
function Format-Repertoire {
    param(
        [string]$Courant,
        [string]$Projet
    )

    if ([string]::IsNullOrWhiteSpace($Courant)) {
        return $null
    }

    $courantNet = $Courant.TrimEnd('\', '/')
    $feuille = Split-Path -Leaf $courantNet

    if ([string]::IsNullOrWhiteSpace($Projet)) {
        return $feuille
    }

    $projetNet = $Projet.TrimEnd('\', '/')
    if ($courantNet -eq $projetNet) {
        return (Split-Path -Leaf $projetNet)
    }

    # Comparaison insensible à la casse : sous Windows deux écritures du même
    # chemin ne diffèrent que par la casse, et rien ne garantit que Claude Code
    # renvoie « current_dir » et « project_dir » dans la même.
    $prefixe = $projetNet + [System.IO.Path]::DirectorySeparatorChar
    if (-not $courantNet.StartsWith(
            $prefixe, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $feuille
    }

    $relatif = $courantNet.Substring($prefixe.Length) -split '[\\/]+' |
        Where-Object { $_ }
    $segments = @(Split-Path -Leaf $projetNet) + @($relatif)

    return ((Compress-Chemin $segments) -join "\")
}

# Met en forme un instant : l'heure seule s'il tombe aujourd'hui, la date
# devant sinon. Les deux instants affichés — remise à zéro et épuisement —
# suivent la même règle.
function Format-Instant {
    param($Instant)

    $local = $Instant.ToLocalTime()
    $format = if ($local.Date -eq (Get-Date).Date) { "HH:mm" } else { "dd/MM HH:mm" }

    return $local.ToString(
        $format, [System.Globalization.CultureInfo]::InvariantCulture)
}

# Rend la famille du modèle de la session, ou $null si elle est inconnue.
#
# Lue dans model.id d'abord — « claude-fable-5-1 » —, puis dans
# model.display_name en repli, pour un payload qui n'aurait que le nom. La
# famille est le premier mot de $FacteursModele que le nom contient, la casse
# ignorée ; un modèle hors table rend $null, et la ligne se comporte alors
# comme avant cette évolution.
function Get-FamilleModele {
    param($Donnees)

    foreach ($nom in @($Donnees.model.id, $Donnees.model.display_name)) {
        if ($nom -isnot [string] -or -not $nom) {
            continue
        }
        $minuscule = $nom.ToLowerInvariant()
        foreach ($entree in $script:FacteursModele) {
            if ($minuscule.Contains($entree.Famille)) {
                return $entree.Famille
            }
        }
    }

    return $null
}

# Facteur de consommation d'une famille, rapporté à Opus : 1 pour une famille
# inconnue ou absente.
function Get-FacteurFamille {
    param([string]$Famille)

    if (-not $Famille) {
        return 1.0
    }
    foreach ($entree in $script:FacteursModele) {
        if ($entree.Famille -ceq $Famille) {
            return [double]$entree.Facteur
        }
    }
    return 1.0
}

# Abaisse un seuil selon le facteur, sans jamais le relever.
#
# Un seuil laisse une réserve — les points qui le séparent du plafond — et
# c'est elle qui donne le temps de lever le pied. Sous un modèle qui consomme
# $Facteur fois plus vite, la même réserve dure $Facteur fois moins longtemps :
# pour garder le même délai, elle doit être $Facteur fois plus large, et le
# seuil descend d'autant. 75 % devient 50 % sous un facteur 2, 85 % devient
# 70 %.
#
# Un facteur inférieur à 1 ne relève rien. Le compteur est partagé entre les
# modèles, et une session Sonnet ne rend pas moins cher ce que Fable a déjà
# consommé la même semaine ; abaisser le seuil est une précaution sur ce qui
# vient, le relever effacerait une alerte sur ce qui est déjà là.
function ConvertTo-SeuilAdapte {
    param(
        [int]$Seuil,
        [double]$Facteur
    )

    if ($Facteur -le 1) {
        return $Seuil
    }
    $reserve = [math]::Round((100 - $Seuil) * $Facteur)
    return [int][math]::Max(100 - $reserve, 0)
}

# Rend le descripteur d'une fenêtre tel qu'il s'applique sous ce facteur.
#
# Seules les fenêtres qui déclarent SelonModele bougent ; les autres reviennent
# telles quelles, ce qui garde à la fenêtre de 5 heures les seuils écrits dans
# $Fenetres.
function Get-DescripteurEffectif {
    param(
        $Descripteur,
        [double]$Facteur
    )

    if (-not $Descripteur.SeuilsSelonModele) {
        return $Descripteur
    }

    return [pscustomobject]@{
        Cle                = $Descripteur.Cle
        Libelle            = $Descripteur.Libelle
        Seuil              = ConvertTo-SeuilAdapte -Seuil $Descripteur.Seuil -Facteur $Facteur
        SeuilCritique      = ConvertTo-SeuilAdapte -Seuil $Descripteur.SeuilCritique -Facteur $Facteur
        ResetToujours      = $Descripteur.ResetToujours
        SeuilsSelonModele  = $Descripteur.SeuilsSelonModele
        AncrageSelonModele = $Descripteur.AncrageSelonModele
    }
}

# Rend la famille mémorisée avec un ancrage, ou $null si le cache n'en porte
# pas — cache antérieur au 03/09/2026, ou fenêtre qui ne suit pas le modèle.
function Get-FamilleMemorisee {
    param($Memorisee)

    $famille = $Memorisee.observe_modele
    if ($famille -is [string] -and $famille.Trim()) {
        return $famille
    }
    return $null
}

# Détermine l'ancrage de mesure d'une fenêtre fraîche : l'instant et le
# pourcentage de la première observation faite sur cette fenêtre-là, et — pour
# les fenêtres qui suivent le modèle — la famille sous laquelle cette
# observation a été faite.
#
# Une fenêtre est identifiée par son heure de remise à zéro. Tant qu'elle ne
# bouge pas, l'ancrage mémorisé est conservé et la durée d'observation
# s'allonge — y compris d'une session à l'autre, ce qui rend la pente
# hebdomadaire exploitable là où une seule session ne mesurerait rien.
#
# Quatre situations réancrent sur l'instant présent, parce que l'ancrage
# mémorisé ne décrit plus la même chose :
#   - remise à zéro différente : c'est une autre fenêtre ;
#   - pourcentage en recul : le compteur ne redescend pas au fil d'une fenêtre,
#     une correction côté serveur invaliderait la pente ;
#   - ancrage postérieur à maintenant : l'horloge du poste a reculé entre deux
#     sessions, et la durée d'observation serait négative ;
#   - famille de modèle différente (03/09/2026), pour les seules fenêtres qui
#     suivent le modèle : une pente mesurée sous Opus ne dit rien de ce que
#     Fable va consommer, à deux fois le prix du jeton. Le segment se tait
#     alors le temps de réobserver, ce qui est le comportement voulu — une
#     projection fausse coûte plus cher qu'un segment absent.
#
# Le réancrage par famille demande les deux familles : la courante et la
# mémorisée. Un modèle hors table, ou un cache antérieur au champ, n'est pas un
# changement mais une inconnue, et l'ancrage tient. Un ancrage conservé garde
# sa famille ; s'il n'en avait pas, il prend celle du jour, faute de mieux.
function Get-Ancrage {
    param(
        $Fraiche,
        $Memorisee,
        [long]$Maintenant,
        [string]$Famille,
        [switch]$SelonModele
    )

    $pourcent = ConvertTo-Pourcent $Fraiche.used_percentage

    $familleNeuve = if ($SelonModele -and $Famille) { $Famille } else { $null }

    $neuf = [pscustomobject]@{
        observe_a        = $Maintenant
        observe_pourcent = $pourcent
        observe_modele   = $familleNeuve
    }

    if ($null -eq $Memorisee -or $null -eq $pourcent) {
        return $neuf
    }

    $reset = ConvertTo-Instant $Fraiche.resets_at
    $resetMemorise = ConvertTo-Instant $Memorisee.resets_at
    if ($null -eq $reset -or $null -eq $resetMemorise -or
        $reset.ToUnixTimeSeconds() -ne $resetMemorise.ToUnixTimeSeconds()) {
        return $neuf
    }

    $ancre = ConvertTo-Nombre $Memorisee.observe_a
    $ancrePourcent = ConvertTo-Pourcent $Memorisee.observe_pourcent
    if ($null -eq $ancre -or $null -eq $ancrePourcent) {
        return $neuf
    }

    if ($pourcent -lt $ancrePourcent -or $ancre -gt $Maintenant) {
        return $neuf
    }

    $conservee = Get-FamilleMemorisee $Memorisee
    if ($SelonModele -and $Famille -and $conservee -and $Famille -cne $conservee) {
        return $neuf
    }

    return [pscustomobject]@{
        observe_a        = [long]$ancre
        observe_pourcent = $ancrePourcent
        observe_modele   = if (-not $SelonModele) {
            $null
        } elseif ($conservee) {
            $conservee
        } else {
            $familleNeuve
        }
    }
}

# Met en forme le rythme de consommation d'une fenêtre, ou $null quand il n'y a
# rien d'honnête à en dire.
#
# La pente est mesurée entre l'ancrage et maintenant, jamais déduite d'une durée
# de fenêtre supposée : rien dans le payload ne dit qu'une fenêtre « five_hour »
# dure cinq heures pleines, et l'hypothèse se paierait sur toutes les valeurs
# affichées.
#
# Deux affichages, exclusifs l'un de l'autre par construction — la projection
# atteint 100 % exactement quand l'épuisement précède la remise à zéro :
#   - « → 61% », consommation prévue à la fin de la fenêtre, soumise aux mêmes
#     paliers de coloration que le pourcentage courant ;
#   - « épuisé 14:10 », heure prévue du plafond, toujours au palier critique :
#     c'est le seul cas où la ligne annonce quelque chose qui n'est pas encore
#     arrivé.
#
# Se retire quand la mesure ne tient pas : observation trop courte dans l'absolu
# ou trop courte au regard de la durée à projeter, rien de consommé depuis
# l'ancrage, fenêtre déjà expirée, ou projection qui, une fois arrondie,
# n'apprend rien de plus que le pourcentage courant.
#
# Se taire est ici le comportement utile, pas un pis-aller : une projection
# fausse mais affirmée coûte plus cher qu'un segment absent, puisqu'elle porte
# la couleur du palier critique et finit par déprécier celle-ci partout
# ailleurs sur la ligne.
function Format-Rythme {
    param(
        $Fenetre,
        $Ancrage,
        $Descripteur,
        [long]$Maintenant
    )

    if ($null -eq $Ancrage -or $null -eq $Ancrage.observe_pourcent) {
        return $null
    }

    $pourcent = ConvertTo-Pourcent $Fenetre.used_percentage
    $reset = ConvertTo-Instant $Fenetre.resets_at
    if ($null -eq $pourcent -or $null -eq $reset) {
        return $null
    }

    $ecoule = $Maintenant - [long]$Ancrage.observe_a
    if ($ecoule -lt ($script:SpanMinimumRythme * 60)) {
        return $null
    }

    $consomme = $pourcent - $Ancrage.observe_pourcent
    if ($consomme -le 0) {
        return $null
    }

    $restant = $reset.ToUnixTimeSeconds() - $Maintenant
    if ($restant -le 0) {
        return $null
    }

    # La durée à projeter doit rester du même ordre que la durée observée.
    # Multiplication plutôt que division : $ecoule a déjà franchi le plancher
    # en minutes, donc jamais zéro, mais la forme multiplicative reste juste
    # même si ce plancher venait à être abaissé.
    if ($restant -gt ($ecoule * $script:RatioMaxExtrapolation)) {
        return $null
    }

    # Pente en points de pourcentage par seconde.
    $pente = $consomme / [double]$ecoule
    $projection = $pourcent + $pente * $restant

    if ($projection -ge 100) {
        # Borné par $restant, puisqu'on n'entre ici que si la projection
        # atteint 100 % avant la remise à zéro.
        $secondes = [long]((100 - $pourcent) / $pente)
        $epuisement = [System.DateTimeOffset]::FromUnixTimeSeconds(
            $Maintenant + $secondes)

        # Seul fragment intégralement coloré, préfixe compris : il n'annonce pas
        # une mesure mais une échéance, et son libellé fait partie de
        # l'avertissement plutôt que du chrome.
        #
        # Au palier critique quel que soit le pourcentage courant : dire que le
        # plafond tombera avant la remise à zéro, c'est déjà annoncer un 100 %,
        # et aucun palier mesuré ne va plus loin.
        return (Add-Couleur ($script:PrefixeEpuisement + " " +
                (Format-Instant $epuisement)) $script:CodeCritique)
    }

    $arrondie = [int][math]::Round($projection)
    if ($arrondie -le $pourcent) {
        return $null
    }

    return (Format-Mesure -Libelle $script:PrefixeProjection `
            -Valeur "$arrondie" -Unite $script:UnitePourcent -Pourcent $arrondie `
            -Seuil $Descripteur.Seuil -SeuilCritique $Descripteur.SeuilCritique)
}

# Met en forme une fenêtre de limitation : pourcentage consommé, rythme, puis
# heure de remise à zéro. $Descripteur est l'entrée de $Fenetres décrivant la
# fenêtre.
#
# L'heure de remise à zéro est préférée à un décompte restant : elle ne dépend
# pas de la fréquence de rafraîchissement de la ligne, et se planifie plus
# directement. La date n'accompagne l'heure que si la remise à zéro tombe un
# autre jour, ce qu'une fenêtre de 5 heures entamée en soirée provoque — et ce
# qu'une fenêtre de 7 jours provoque presque toujours.
#
# Elle n'est affichée que si le descripteur l'exige, ou si le seuil d'alerte
# est franchi : voir ResetToujours en tête de fichier. Un pourcentage
# inexploitable retire donc la fenêtre entière, heure comprise — sans le
# pourcentage, elle ne situe plus rien.
#
# -Memorisee marque le pourcentage d'un « ~ » : la valeur vient du cache et non
# du payload de la session en cours, et ne doit pas passer pour une mesure
# fraîche. Le rythme est alors tu : projeter à partir d'une mesure déjà périmée
# empilerait deux approximations.
function Format-Fenetre {
    param(
        $Fenetre,
        $Descripteur,
        $Ancrage,
        [long]$Maintenant,
        [switch]$Memorisee,
        # Pourcentage estimé par dérive — 05/09/2026 —, qui remplace à
        # l'affichage celui de la fenêtre : marqué « ≈ », sans rythme. $null
        # quand il n'y a rien à estimer. Voir Get-DeriveModele.
        $Estimation
    )

    if ($null -eq $Fenetre) {
        return $null
    }

    $mesure = ConvertTo-Pourcent $Fenetre.used_percentage
    if ($null -eq $mesure) {
        return $null
    }

    # Une estimation remplace la mesure et la marque ; elle l'emporte sur une
    # valeur mémorisée — estimée, la valeur n'est plus ancienne. Dans les deux
    # cas la valeur est figée : le rythme se tait, projeter depuis une mesure
    # déjà périmée, ou déjà estimée, empilerait deux approximations.
    if ($null -ne $Estimation) {
        $pourcent = [int]$Estimation
        $prefixe = $script:MarqueurEstimee
        $figee = $true
    }
    elseif ($Memorisee) {
        $pourcent = $mesure
        $prefixe = $script:MarqueurMemorisee
        $figee = $true
    }
    else {
        $pourcent = $mesure
        $prefixe = ""
        $figee = $false
    }

    $enAlerte = $pourcent -ge $Descripteur.Seuil

    # Le marqueur reste collé à la valeur : il qualifie la mesure, pas le
    # compteur. La jauge, elle, précède les deux : elle annonce le remplissage
    # que le nombre chiffre, et vaut pour une valeur mémorisée ou estimée comme
    # pour une mesure fraîche — celle du cache est ancienne, pas fausse.
    $morceaux = @(
        Format-Mesure -Libelle $Descripteur.Libelle `
            -Valeur "$prefixe$pourcent" -Unite $script:UnitePourcent `
            -Pourcent $pourcent `
            -Seuil $Descripteur.Seuil -SeuilCritique $Descripteur.SeuilCritique `
            -Jauge
    )

    if (-not $figee) {
        $rythme = Format-Rythme -Fenetre $Fenetre -Ancrage $Ancrage `
            -Descripteur $Descripteur -Maintenant $Maintenant
        if ($rythme) {
            $morceaux += $rythme
        }
    }

    if ($Descripteur.ResetToujours -or $enAlerte) {
        $instant = ConvertTo-Instant $Fenetre.resets_at
        if ($null -ne $instant) {
            # Atténuée : c'est un repère de planification, pas une valeur à
            # surveiller.
            #
            # Le mot « reset » qui la précédait est tombé le 25/08/2026. Une
            # heure qui suit un pourcentage, dans un segment de fenêtre, ne peut
            # désigner qu'une remise à zéro — et le mot coûtait six colonnes par
            # fenêtre, doublées dans le cas d'alerte, c'est-à-dire là où la ligne
            # est déjà à son plus long. L'heure d'épuisement, elle, garde son
            # libellé et sa couleur critique : il y porte un avertissement, pas
            # une étiquette.
            $morceaux += Add-Attenuation (Format-Instant $instant)
        }
    }

    # Le pourcentage accompagne le segment depuis le soir du 03/09/2026, et le
    # palier franchi depuis le 04/09/2026 : c'est sur eux que deux fenêtres
    # candidates à la même place sur la ligne se départagent — voir
    # Get-SegmentFenetres.
    return [pscustomobject]@{
        Segment  = ($morceaux -join $script:SeparateurInterne)
        Pourcent = $pourcent
        EnAlerte = $enAlerte
    }
}

# Produit le segment d'emplacement : répertoire, et branche entre parenthèses
# quand il y en a une.
#
# C'est ce segment qui est posé en pastille, seule mise en valeur par le fond de
# la ligne : sur un workspace multi-projets, savoir d'où l'on parle prime sur
# tout le reste, et c'est la seule information de la ligne que l'œil doive
# retrouver sans la chercher. Le raisonnement qui cantonne le fond à un unique
# segment est en tête de $CodePastille.
function Get-SegmentEmplacement {
    param($Donnees)

    # « cwd » double « workspace.current_dir » et sert de repli si le contrat
    # d'entrée venait à ne plus porter l'objet « workspace ».
    $courant = if ($Donnees.workspace.current_dir) {
        $Donnees.workspace.current_dir
    } else {
        $Donnees.cwd
    }

    $repertoire = Format-Repertoire -Courant $courant `
        -Projet $Donnees.workspace.project_dir

    if (-not $repertoire) {
        return $null
    }

    # « worktree.branch » n'existe que pour les sessions --worktree ; partout
    # ailleurs la branche se lit sur le disque.
    $branche = if ($Donnees.worktree.branch) {
        $Donnees.worktree.branch
    } else {
        Get-BrancheGit $courant
    }

    $emplacement = if ($branche) { "$repertoire ($branche)" } else { $repertoire }

    # Le fond couvre le segment entier, branche comprise : ce qui est marqué,
    # c'est l'emplacement, pas l'une de ses deux moitiés. Un fond qui s'arrêterait
    # au répertoire ferait lire la branche comme un segment à part, alors que
    # « · » est le seul séparateur de ce rang.
    return (Add-Pastille $emplacement)
}

# Produit le segment d'abonnement, tout en tête de ligne et en pastille.
#
# Il ouvre la ligne parce qu'il en est le cadre : le modèle dit ce qui tourne,
# et l'abonnement sous quel régime — c'est lui qui décide de la taille des
# fenêtres de limitation affichées à l'autre bout.
#
# Il a été atténué comme la version, et pour la même raison : c'est l'information
# la moins volatile de toute la ligne, celle qui se consulte et ne se surveille
# pas. Il occupe désormais la pastille de tête, celle que le crabe « (V)°°(V) » a
# portée quelques heures — même teinte, même fond épousant le texte, même espace
# de séparation hors du fond. Voir Add-PastilleAbonnement.
#
# L'échange se justifie par ce que chacun occupait : le logo prenait huit
# colonnes pour ne rien dire de la session, l'abonnement en prend trois et dit le
# régime sous lequel elle tourne. La marque du produit reste — c'est sa teinte
# qui peint la pastille — mais elle porte maintenant un mot.
#
# Une conséquence à noter : le logo était posé après Confirm-Ligne et paraissait
# donc sur les lignes de repli, y compris quand l'assemblage avait échoué.
# L'abonnement, lui, est un segment : sur un payload illisible, la ligne se
# réduit au nom du modèle, sans pastille. C'est cohérent avec ce qu'il est — une
# information, pas une marque —, mais la ligne dégradée n'a plus rien qui la
# signe.
function Get-SegmentAbonnement {
    param($Config)

    $abonnement = Get-Abonnement -Config $Config

    if ([string]::IsNullOrWhiteSpace($abonnement)) {
        return $null
    }

    return (Add-PastilleAbonnement $abonnement)
}

# Produit le segment de modèle : nom, effort de raisonnement, puis marqueurs de
# mode. Seul segment à ne jamais rendre $null, $Modele portant déjà un repli.
#
# L'effort est accolé au nom plutôt que posé en segment propre : il qualifie le
# modèle. Il mérite sa place parce qu'il se règle à trois endroits — settings,
# drapeau de lancement, « /effort » en cours de session — dont le dernier ne
# laisse aucune trace ailleurs dans l'interface, et parce que c'est lui qui
# gouverne la vitesse à laquelle se remplissent les fenêtres affichées plus
# loin. Le payload l'omet pour les modèles qui ne prennent pas ce paramètre.
#
# Les deux marqueurs de mode ne s'affichent que sur l'état inhabituel — mode
# rapide actif, réflexion étendue coupée — dans le même esprit que les segments
# qui se retirent : la ligne ne porte que ce qui s'écarte de l'ordinaire.
#
# Les trois champs sont contrôlés en type avant lecture. Un booléen comparé
# directement se passerait mal d'un payload dégénéré : « -eq » sur un tableau
# filtre au lieu de comparer, et rendrait une valeur vraie pour de mauvaises
# raisons.
function Get-SegmentModele {
    param(
        $Donnees,
        [string]$Modele
    )

    $morceaux = @($Modele)

    # L'effort est atténué : il qualifie le modèle et se consulte, là où le nom
    # du modèle identifie la session. Les deux marqueurs qui suivent prennent la
    # teinte des marqueurs — ils ne s'affichent que sur l'état inhabituel, et les
    # atténuer irait contre la raison même de leur présence.
    $effort = $Donnees.effort.level
    if ($effort -is [string] -and -not [string]::IsNullOrWhiteSpace($effort)) {
        $morceaux += Add-Attenuation $effort.Trim()
    }

    # Teintés depuis le 25/08/2026, au lieu de la couleur pleine du thème qui
    # était aussi celle du nom du modèle : les deux se lisaient comme un seul
    # nom, alors que la moitié droite décrit la session. Voir $CodeMarqueur.
    if ($Donnees.fast_mode -is [bool] -and $Donnees.fast_mode) {
        $morceaux += Add-Marqueur $script:LibelleModeRapide
    }

    if ($Donnees.thinking.enabled -is [bool] -and -not $Donnees.thinking.enabled) {
        $morceaux += Add-Marqueur $script:LibelleSansReflexion
    }

    return ($morceaux -join " ")
}

# Produit le segment d'occupation du contexte.
#
# « used_percentage » est nul avant le premier appel API, et de nouveau après
# un /compact tant que rien n'a été renvoyé : le segment disparaît alors,
# plutôt que d'afficher un « ctx 0% » qui se lirait comme une mesure.
function Get-SegmentContexte {
    param($Donnees)

    $contexte = $Donnees.context_window
    if ($null -eq $contexte) {
        return $null
    }

    $pourcent = ConvertTo-Pourcent $contexte.used_percentage
    if ($null -eq $pourcent) {
        return $null
    }

    return (Format-Mesure -Libelle "ctx" -Valeur "$pourcent" `
            -Unite $script:UnitePourcent `
            -Pourcent $pourcent -Seuil $script:SeuilContexte `
            -SeuilCritique $script:SeuilCritiqueContexte)
}

# Produit les segments des fenêtres de limitation, et mémorise au passage les
# valeurs fraîches pour les prochains lancements de session.
#
# Le cache est relu une seule fois pour les deux fenêtres, et réécrit une seule
# fois : la ligne se rafraîchit souvent, et chaque fenêtre traitée séparément
# doublerait les accès disque sans rien apporter.
#
# Les fenêtres reprises du cache y sont remises telles quelles. C'est ce qui
# préserve « seven_day » quand le payload ne porte que « five_hour », les deux
# pouvant être absentes indépendamment l'une de l'autre.
#
# Le modèle de la session entre ici (03/09/2026). Sa famille donne le facteur
# de consommation, le facteur donne à chaque fenêtre qui le suit son
# descripteur effectif — seuils abaissés sous Fable —, et la famille elle-même
# va à l'ancrage, qui repart quand elle change. Une fenêtre relue du cache est
# colorée aux seuils du jour : ils décrivent ce que la session va consommer,
# pas ce qu'une autre a consommé.
# Et sa fenêtre propre, le soir même. Quand /usage a relevé une fenêtre
# hebdomadaire pour cette famille — voir Get-FenetreModele —, elle est mesurée
# comme les autres, sous sa propre clé de cache et à l'instant de son relevé,
# puis elle se dispute la place « 7j » avec la fenêtre du payload. Sur ce
# poste le 03/09/2026 au soir, 28 % pour Fable contre 14 % pour tous les
# modèles — et la ligne dit 28, comme /usage ; le lendemain, 0 % contre 2 %,
# et la ligne dit 0, comme /usage encore.
#
# La règle d'attribution a changé le 04/09/2026. La place allait jusque-là à
# la plus remplie, sans condition, au motif que c'est elle qui bornera la
# session la première. Le lendemain d'une remise à zéro hebdomadaire, cela
# donnait la place à une fenêtre globale à 2 % contre une fenêtre Fable à 0 % :
# la ligne ne disait plus rien du modèle qui tourne, alors que sa fenêtre se
# remplit deux fois plus vite que la globale et la rattrape en une heure de
# travail. Désormais la fenêtre du modèle prend la place tant que la globale
# n'a rien à signaler ; dès que celle-ci alerte, elle reprend la place si elle
# est la plus remplie — le cas qui avait motivé l'ancienne règle, et que la
# nouvelle couvre encore. Voir Get-SegmentFenetres.

# Mesure une fenêtre fraîche : pose ou conserve son ancrage, la remet au cache,
# et la met en forme. $Descripteur porte les règles d'ancrage et la clé de
# cache, $Effectif les seuils tels qu'ils s'appliquent sous le modèle courant —
# les deux ne diffèrent que pour la fenêtre hebdomadaire du payload. -Ancienne
# fait afficher la valeur comme une valeur de cache, sans rythme : c'est le cas
# d'un relevé de /usage vieux de plus d'une heure, que l'on ancre et mémorise
# quand même, puisque la mesure est réelle.
function Measure-Fenetre {
    param(
        $Fraiche,
        $Descripteur,
        $Effectif,
        $Memorisee,
        [long]$Maintenant,
        [string]$Famille,
        [switch]$Ancienne,
        $AMemoriser,
        # Pourcentage estimé par dérive, $null sinon : transmis tel quel à
        # Format-Fenetre, le cache n'en sait rien. Voir Get-DeriveModele.
        $Estimation
    )

    $ancrage = Get-Ancrage -Fraiche $Fraiche -Memorisee $Memorisee `
        -Maintenant $Maintenant -Famille $Famille `
        -SelonModele:([bool]$Descripteur.AncrageSelonModele)

    $AMemoriser[$Descripteur.Cle] = [pscustomobject]@{
        used_percentage  = $Fraiche.used_percentage
        resets_at        = $Fraiche.resets_at
        observe_a        = $ancrage.observe_a
        observe_pourcent = $ancrage.observe_pourcent
        observe_modele   = $ancrage.observe_modele
    }

    return (Format-Fenetre -Fenetre $Fraiche -Descripteur $Effectif `
            -Ancrage $ancrage -Maintenant $Maintenant -Memorisee:$Ancienne `
            -Estimation $Estimation)
}

function Get-SegmentFenetres {
    param(
        $Donnees,
        $Config
    )

    # Un seul instant de référence pour tout le passage : la pente affichée et
    # l'ancrage écrit dans le cache doivent parler de la même seconde.
    $maintenant = [System.DateTimeOffset]::UtcNow.ToUnixTimeSeconds()

    $famille = Get-FamilleModele $Donnees
    $facteur = Get-FacteurFamille $famille

    $cache = Get-CacheFenetres
    $aMemoriser = [ordered]@{}

    # Une place par libellé, dans l'ordre des fenêtres du payload.
    $places = [ordered]@{}

    foreach ($descripteur in $script:Fenetres) {
        $effectif = Get-DescripteurEffectif -Descripteur $descripteur -Facteur $facteur
        $fraiche = $Donnees.rate_limits.($descripteur.Cle)
        $memorisee = Get-FenetreMemorisee -Cache $cache -Cle $descripteur.Cle

        $candidat = $null
        if ($null -ne $fraiche -and $null -ne $fraiche.used_percentage) {
            $candidat = Measure-Fenetre -Fraiche $fraiche -Descripteur $descripteur `
                -Effectif $effectif -Memorisee $memorisee -Maintenant $maintenant `
                -Famille $famille -AMemoriser $aMemoriser
        }
        elseif ($null -ne $memorisee) {
            # Avant la première requête de la session, « rate_limits » n'est
            # pas renseigné : on retombe sur la dernière valeur connue.
            # L'ancrage mémorisé est réécrit tel quel, famille comprise, faute
            # de nouvelle mesure à y verser.
            $aMemoriser[$descripteur.Cle] = $memorisee
            $candidat = Format-Fenetre -Fenetre $memorisee `
                -Descripteur $effectif -Maintenant $maintenant -Memorisee
        }

        $places[$descripteur.Libelle] = $candidat
    }

    $releve = Get-FenetreModele -Config $Config -Famille $famille -Maintenant $maintenant
    if ($null -ne $releve) {
        $descripteur = $script:FenetreModele
        $ancienne = ($maintenant - $releve.releve_a) -gt $script:AgeMaxUsagePersiste
        $fraiche = [pscustomobject]@{
            used_percentage = $releve.used_percentage
            resets_at       = $releve.resets_at
        }
        $memorisee = Get-FenetreMemorisee -Cache $cache -Cle $descripteur.Cle

        # La fenêtre du payload qui partage la place — la globale de la
        # semaine — est fraîche à chaque réponse : ce qu'elle a pris depuis le
        # relevé, la fenêtre du modèle l'a pris aussi. Voir Get-DeriveModele.
        $globale = $null
        foreach ($f in $script:Fenetres) {
            if ($f.Libelle -ceq $descripteur.Libelle) {
                $globale = $Donnees.rate_limits.($f.Cle)
                break
            }
        }
        $estimation = Get-DeriveModele -Releve $releve -Globale $globale

        # Mesurée à l'instant de son relevé, et non maintenant : c'est à cet
        # instant-là que le pourcentage était vrai, et la pente comme la
        # projection se calculent dans ce repère.
        $candidat = Measure-Fenetre -Fraiche $fraiche -Descripteur $descripteur `
            -Effectif $descripteur -Memorisee $memorisee -Maintenant $releve.releve_a `
            -Famille $famille -Ancienne:$ancienne -AMemoriser $aMemoriser `
            -Estimation $estimation

        # La place va à la fenêtre du modèle — la nouvelle venue, celle que
        # /usage attribue à la session —, sauf si l'occupante, la fenêtre
        # globale, est en alerte et strictement plus remplie : c'est alors elle
        # qui menace la session, et la cacher coûterait l'alerte. À égalité, la
        # fenêtre du modèle. Règle du 04/09/2026 ; le commentaire en tête de
        # Measure-Fenetre dit ce qu'elle remplace, et pourquoi.
        if ($null -ne $candidat) {
            $occupant = $places[$descripteur.Libelle]
            if ($null -eq $occupant -or
                -not ($occupant.EnAlerte -and $occupant.Pourcent -gt $candidat.Pourcent)) {
                $places[$descripteur.Libelle] = $candidat
            }
        }
    }

    Save-Fenetres $aMemoriser

    $segments = @()
    foreach ($place in $places.Values) {
        if ($null -ne $place -and $place.Segment) {
            $segments += $place.Segment
        }
    }

    return ($segments -join (Get-Separateur))
}

# ===========================================================================
# 7. Assemblage
# ===========================================================================

# Extrait le nom du modèle. Ne lève jamais : le résultat sert aussi de repli
# si l'assemblage échoue plus loin.
#
# Repli sur un libellé neutre : le modèle est toujours présent en pratique,
# mais la ligne doit rester lisible si le contrat d'entrée évolue.
function Get-Modele {
    param($Donnees)

    if ($Donnees.model.display_name) {
        return $Donnees.model.display_name
    }

    return $script:ModeleParDefaut
}

# Assemble la ligne complète, en quatre groupes. Les segments $null se retirent
# d'eux-mêmes, et un groupe qui perd tous les siens disparaît avec eux — son
# séparateur compris, sans quoi la ligne garderait deux espaces là où le groupe
# se tenait.
#
# Les familles sont celles que la ligne décrivait déjà sans le dire : sous quel
# régime (l'abonnement), ce qui tourne, où l'on est, ce que la session consomme.
# Voir $SeparateurGroupe.
#
# L'abonnement a pris son propre groupe le 26/08/2026 au soir, en même temps que
# sa pastille. Il ouvrait jusque-là le groupe « ce qui tourne », d'où un « · »
# entre lui et la version ; un fond n'a pas besoin de ce point pour qu'on voie où
# il s'arrête, et la mécanique des groupes lui donne au passage le bon
# comportement lorsqu'il manque — la ligne commence alors par le modèle, sans
# espace orphelin. La version, qui précédait le modèle, est retirée depuis le
# 23/09/2026.
function Get-LigneStatut {
    param(
        $Donnees,
        [string]$Modele
    )

    # Une seule lecture de « ~/.claude.json » pour les deux segments qui s'y
    # servent : l'abonnement, et la fenêtre propre au modèle.
    $config = Get-Configuration

    # Chaque groupe est un tableau de segments. La virgule de tête protège les
    # groupes d'un seul élément : sans elle, PowerShell aplatirait le tableau et
    # la boucle recevrait des chaînes au lieu de groupes.
    $groupes = @(
        , @( (Get-SegmentAbonnement -Config $config) )
        , @( (Get-SegmentModele -Donnees $Donnees -Modele $Modele) )
        , @( (Get-SegmentEmplacement $Donnees) )
        , @(
            (Get-SegmentContexte $Donnees)
            (Get-SegmentFenetres -Donnees $Donnees -Config $config)
        )
    )

    $separateur = Get-Separateur

    $blocs = foreach ($groupe in $groupes) {
        $retenus = @($groupe | Where-Object { $_ })
        if ($retenus.Count -gt 0) {
            $retenus -join $separateur
        }
    }

    return (($blocs | Where-Object { $_ }) -join $script:SeparateurGroupe)
}

# ===========================================================================
# 8. Programme principal
# ===========================================================================
#
# Un seul invariant gouverne ce bloc : **toujours une ligne non blanche, et
# toujours le code de sortie 0**. Claude Code ne conserve pas le texte
# précédent — il pousse le résultat de chaque lancement tel quel, y compris
# vide, dès que la sortie est blanche ou que le code de sortie n'est pas nul.
# Une seule exécution muette efface donc la ligne, et rien ne la ramène avant
# le prochain rafraîchissement. Voir le README, « Pourquoi la ligne
# disparaissait ».

$brut = ""
try {
    $brut = [Console]::In.ReadToEnd()
}
catch {
    # Entrée illisible : il ne reste rien à afficher, mais se taire coûterait
    # la ligne. Le libellé neutre en tient la place.
    Write-Repli $ModeleParDefaut
    exit 0
}

# Payload absent ou blanc. Le cas ne devrait pas se produire — Claude Code
# écrit le JSON sur l'entrée standard avant de la fermer — mais cette écriture
# peut échouer sans que le script en sache rien : le tube rompu est signalé à
# l'appelant, pas au processus lancé, qui ne voit qu'une entrée vide. Le repli
# n'invente aucune information et préserve la ligne.
if ([string]::IsNullOrWhiteSpace($brut)) {
    Write-Repli $ModeleParDefaut
    exit 0
}

$payload = $brut.Trim()

try {
    $donnees = $brut | ConvertFrom-Json
}
catch {
    Write-Diagnostic -Erreur $_ -Payload $payload
    Write-Repli $ModeleParDefaut
    exit 0
}

# Le modèle est résolu à part, avant tout ce qui peut échouer : il tient lieu
# de repli si l'assemblage lève.
$modele = Get-Modele $donnees

try {
    $ligne = Get-LigneStatut -Donnees $donnees -Modele $modele
    Write-LigneStatut (Confirm-Ligne -Ligne $ligne -Repli $modele)
}
catch {
    Write-Diagnostic -Erreur $_ -Payload $payload
    Write-Repli (Confirm-Ligne -Ligne $modele -Repli $ModeleParDefaut)
}

# Sortie explicite : une erreur terminante qui remonterait jusqu'ici ferait
# rendre 1 à PowerShell, ce que Claude Code compte en « nonzero_exit » et
# traite comme une ligne vide.
exit 0
