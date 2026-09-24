//! Réglages : seuils, couleurs, libellés et noms de fichiers.
//!
//! Section 1 du script d'origine. Tout ce qui se règle sans toucher à une
//! logique vit ici et nulle part ailleurs : un seuil ou une couleur écrite en
//! dur au milieu d'une mise en forme échapperait à toute relecture d'ensemble.

/// Séparateur inséré entre deux segments d'un même compartiment. Rang
/// intermédiaire des trois : au-dessus, la **jonction** de deux compartiments
/// — voir [`JONCTION`] — ; en dessous, [`SEPARATEUR_INTERNE`].
///
/// C'est le seul des trois rangs à porter un glyphe de ponctuation : le rang
/// supérieur s'écrit par la forme de la capsule, le rang inférieur par un
/// espace, parce qu'un segment n'a pas à s'articuler plus fort que cela. Du
/// 26/08/2026 au 19/09/2026, le rang supérieur était lui aussi un espace, la
/// pastille de l'emplacement bornant son groupe à elle seule — voir
/// `statusline-rust.md` §16 et §23.
pub(crate) const SEPARATEUR: &str = " · ";

/// Séquence SGR du séparateur de segment.
///
/// Ce point médian sortait **sans aucune séquence** jusqu'au matin du
/// 26/08/2026, donc dans la couleur pleine du thème : le séparateur du rang
/// intermédiaire était par là même l'élément le plus lumineux de la zone des
/// compteurs. Il a rejoint le chrome ce jour-là, au gris 245 des libellés.
///
/// **Un cran sous les libellés depuis le 19/09/2026** : il structure, il
/// n'informe pas. L'objection du 26/08 — « deux rangs pour une seule teinte »
/// — visait la barre de groupe, tombée le soir même ; rien ne dispute plus ce
/// rang au point. Sur l'ardoise du corps, 2,7:1 : lisible comme une
/// ponctuation, en retrait des libellés à 4,9:1.
pub(crate) const CODE_SEPARATEUR: &str = "38;2;124;132;142";

/// Séparateur interne à un segment composite : les morceaux d'une même fenêtre
/// de limitation. Il est volontairement plus discret que [`SEPARATEUR`], qui
/// marque le rang supérieur. Cette distinction ne dépend d'aucune couleur :
/// elle survit à `NO_COLOR`, là où la hiérarchie portée par [`CODE_ATTENUE`]
/// disparaît.
pub(crate) const SEPARATEUR_INTERNE: &str = " ";

// ---------------------------------------------------------------------------
// Forme de la capsule — 19/09/2026
// ---------------------------------------------------------------------------
//
// La ligne compte quatre familles — sous quel régime (abonnement), ce qui
// tourne (le modèle), où l'on est (emplacement), ce que la session
// consomme (contexte et fenêtres) — et chacune est désormais un **compartiment**
// d'une capsule, avec son fond. Le rang supérieur de séparation ne s'écrit donc
// plus par un caractère mais par la forme : la jonction de deux fonds.
//
// L'histoire de ce rang tient en trois états, dont le dossier
// `statusline-rust.md` porte le détail : une barre `│` du 25/08 au 26/08/2026
// (§13, §16) ; un espace simple du 26/08 au 19/09/2026, la pastille de
// l'emplacement bornant son groupe à elle seule (§16) ; la jonction depuis
// (§23). Sous `NO_COLOR`, il redevient l'espace simple, et la ligne est celle
// du deuxième état.

/// Liseré posé de part et d'autre du texte de chaque compartiment : un blanc
/// réel, hors de toute séquence, qui hérite du fond ouvert.
///
/// Les pastilles avaient perdu leur liseré le 26/08/2026 au soir : deux
/// colonnes par pastille pour de l'air, sur une grille où un liseré fait une
/// cellule pleine ou rien (`statusline-rust.md` §15). Il revient avec la capsule
/// parce que la forme le demande : un cap arrondi contre une lettre se lit mal,
/// et la jonction de deux fonds a besoin d'une cellule de respiration de chaque
/// côté pour que l'œil sépare les compartiments.
pub(crate) const LISERE: &str = " ";

/// Cap gauche de la capsule : le demi-cercle fin Powerline `U+E0B7`, tracé dans
/// l'encre du contour — [`RVB_CONTOUR`] — sur le fond du terminal.
///
/// Aucune police du poste ne porte ce glyphe — ni Consolas, ni Cascadia Code,
/// ni Cascadia Mono, relevé dans leurs tables le 19/09/2026 — ; c'est Windows
/// Terminal qui le trace lui-même, ce que la commande du chantier a confirmé le
/// jour même sans qu'aucun réglage soit posé. Les demi-blocs `▐` et `▌`, eux
/// dans Consolas, sont le repli connu si un autre terminal ne le traçait pas.
///
/// Le demi-disque plein `U+E0B6`, peint dans la teinte du premier compartiment,
/// a été le premier choix ; il a cédé la place à l'arc fin le 19/09/2026 au
/// soir, sur comparaison des deux à l'écran : un arc n'est qu'un trait, le fond
/// de la cellule reste celui du terminal.
pub(crate) const CAP_GAUCHE: &str = "\u{E0B7}";

/// Cap droit de la capsule : le demi-cercle fin Powerline `U+E0B5`, tracé dans
/// l'encre du contour sur le fond du terminal — voir [`CAP_GAUCHE`].
pub(crate) const CAP_DROIT: &str = "\u{E0B5}";

/// Jonction entre deux compartiments : le même arc que le cap droit, tracé dans
/// l'encre du contour sur le fond du compartiment suivant. Une colonne par
/// jonction, émise même entre deux teintes égales — Q7 du chantier : la
/// géométrie ne dépend pas des teintes.
pub(crate) const JONCTION: &str = "\u{E0B5}";

/// Encre du contour : les deux caps, les jonctions et les deux bords — le gris
/// du point séparateur, [`CODE_SEPARATEUR`].
///
/// Le contour est né le 19/09/2026 au soir, en quatre temps à l'écran : les
/// arcs fins plutôt que les demi-disques ; les arcs en blanc ; un trait dessous
/// et un trait dessus par attributs SGR (`4` + `58`, `53`) pour cerner la
/// pilule — et la capture dans Claude Code qui les a récusés : le surlignement
/// n'y est pas rendu, et le soulignement se place à la hauteur que la police
/// fixe, deux pixels au-dessus du bord. D'où le **cadre sur trois rangs** —
/// voir [`BORD_HAUT`] — et, à la comparaison, le gris plutôt que le blanc.
///
/// Une cellule n'a qu'une encre et un fond : un arc ne peut pas cerner un
/// disque plein, l'intérieur d'un arc est le fond de ce qui suit, et ce sont
/// les bords qui ferment la forme.
pub(crate) const RVB_CONTOUR: &str = "124;132;142";

/// Encres du contour d'une mesure en alerte et au palier critique — 24/09/2026,
/// chantier « mesures », piste B.
///
/// Autour d'une mesure qui a franchi un seuil, les deux bords et l'arc qui la
/// ferme prennent l'encre même de ses valeurs — l'ambre de [`CODE_ALERTE`], le
/// corail de [`CODE_CRITIQUE`] — au lieu du gris de [`RVB_CONTOUR`] : l'alerte
/// se voit du coin de l'œil, sur le cadre, sans une colonne de plus. L'arc qui
/// sépare deux mesures prend l'encre de la plus grave des deux ; une mesure
/// critique bordée d'un arc gris paraissait ouverte.
///
/// Triplets nus, comme [`RVB_CONTOUR`] ; un test vérifie qu'ils restent ceux
/// des deux séquences de palier.
pub(crate) const RVB_CONTOUR_ALERTE: &str = "242;171;63";
pub(crate) const RVB_CONTOUR_CRITIQUE: &str = "249;148;138";

/// Bord haut du cadre : le huitième de bloc inférieur `▁` (`U+2581`), répété
/// sur le rang **au-dessus** de la capsule, d'une colonne après le cap gauche à
/// la colonne avant le cap droit. Posé au bas de sa cellule, il affleure le
/// bord haut de la pilule, là où l'arc du cap se termine.
///
/// Un glyphe de bloc, pas un attribut : Windows Terminal le trace lui-même au
/// bord exact de la cellule, et Claude Code le relaie comme du texte — il
/// rogne chaque ligne (`trim`) avant de l'afficher, ce qui impose d'ouvrir le
/// rang par la séquence d'encre et non par le blanc qui saute le cap.
pub(crate) const BORD_HAUT: &str = "\u{2581}";

/// Bord bas du cadre : le huitième de bloc supérieur `▔` (`U+2594`), sur le
/// rang **au-dessous** de la capsule — voir [`BORD_HAUT`].
pub(crate) const BORD_BAS: &str = "\u{2594}";

/// Cellules retranchées à la largeur de la console pour obtenir la capacité
/// d'un rang — lot 2 du chantier, 19/09/2026 au soir.
///
/// Claude Code n'a pas coupé une ligne plus large que son terminal : il l'a
/// **repliée** sur le rang suivant, cap compris. La ligne se replie donc
/// elle-même, à une frontière de compartiment ou de segment, dès qu'un rang
/// dépasserait la capacité — voir `empaqueter` dans [`crate::sortie`]. On ne
/// sait pas si Claude Code réserve la dernière colonne : une cellule de marge,
/// à passer à deux si l'enroulement subsistait à la largeur exacte.
pub(crate) const MARGE_LARGEUR: usize = 1;

/// Libellé affiché quand le modèle est inconnu, et repli ultime sur erreur.
pub(crate) const MODELE_PAR_DEFAUT: &str = "Claude";

/// Configuration de Claude Code, relative au profil utilisateur : c'est là que
/// se lit l'abonnement du compte connecté, le payload n'en disant rien.
/// `CLAUDE_STATUSLINE_CONFIG` désigne un autre chemin, absolu, sur un poste
/// installé ailleurs. Voir [`crate::abonnement`].
pub(crate) const CHEMIN_CONFIG_RELATIF: &str = ".claude.json";

/// Types d'organisation et libellés d'abonnement correspondants.
///
/// Les quatre clés sont **relevées** dans le bundle JavaScript de `claude.exe`,
/// où Claude Code tient la même correspondance pour son propre usage :
/// `new Map([["claude_max","max"],["claude_pro","pro"],
/// ["claude_enterprise","enterprise"],["claude_team","team"]])`. Elles forment
/// donc l'ensemble complet à la version 2.1.246 — et jusqu'à la prochaine, d'où
/// le repli d'[`embellir`](crate::abonnement) sur toute valeur hors table.
///
/// **Le mot « Claude » est tombé le 26/08/2026**, second lot de retouches. Il
/// occupait sept colonnes en tête de ligne pour ne rien distinguer : dans une
/// ligne de statut de Claude Code, aucun autre éditeur ne dispute le nom. C'est
/// mot pour mot l'argument qui avait réduit « Claude Code v » au « v » seul,
/// appliqué au segment voisin — celui de version, retiré depuis, le
/// 23/09/2026.
///
/// [`embellir`](crate::abonnement) élague le même préfixe sur les valeurs hors
/// table, faute de quoi un abonnement inventé après cette version s'afficherait
/// « Claude Ultra Plus » à côté d'un « Pro » — la table dirait une chose et le
/// repli une autre.
pub(crate) const ABONNEMENTS: [(&str, &str); 4] = [
    ("claude_pro", "Pro"),
    ("claude_max", "Max"),
    ("claude_team", "Team"),
    ("claude_enterprise", "Enterprise"),
];

/// Paliers d'un abonnement Max, et le suffixe accolé au libellé.
///
/// Le palier vit dans un champ distinct du type d'organisation, `5x` et `20x`
/// n'étant pas deux abonnements mais deux volumes du même. Les deux clés
/// viennent du même relevé que [`ABONNEMENTS`], où Claude Code les compare pour
/// décider ce qu'il propose à la vente.
pub(crate) const PALIERS_MAX: [(&str, &str); 2] = [
    ("default_claude_max_5x", "5x"),
    ("default_claude_max_20x", "20x"),
];

/// Seuils de mise en alerte, en pourcent. Distincts parce que les trois
/// compteurs n'appellent pas la même réaction : le contexte se compacte, la
/// fenêtre de 5 heures se subit une poignée d'heures, celle de 7 jours se subit
/// une semaine — d'où un seuil hebdomadaire plus bas, qui laisse le temps de
/// lever le pied avant d'y être.
///
/// Chaque compteur porte deux paliers : franchir le premier appelle à lever le
/// pied, franchir le second à s'arrêter.
pub(crate) const SEUIL_CONTEXTE: i64 = 80;
pub(crate) const SEUIL_CRITIQUE_CONTEXTE: i64 = 90;
pub(crate) const SEUIL_FENETRE_5H: i64 = 80;
pub(crate) const SEUIL_CRITIQUE_FENETRE_5H: i64 = 90;
pub(crate) const SEUIL_FENETRE_7J: i64 = 75;
pub(crate) const SEUIL_CRITIQUE_FENETRE_7J: i64 = 85;

/// Facteurs de consommation par famille de modèle, rapportés à Opus —
/// 03/09/2026.
///
/// Le budget hebdomadaire est unique quel que soit le modèle, mais il se vide
/// au prix du jeton : à travail égal, Fable 5.1 consomme deux fois ce qu'Opus 5
/// consomme. Les valeurs sont le rapport des tarifs à ceux d'Opus 5, relevé sur
/// la grille du 03/09/2026 — le rapport est le même en entrée et en sortie :
/// Fable 5.1 et Mythos 5.1 à 10 $ / 50 $ le million de jetons, Opus 5 à
/// 5 $ / 25 $, Sonnet 5 à 2 $ / 10 $, Haiku 4.5 à 1 $ / 5 $.
///
/// La famille se lit dans `model.id` — `claude-fable-5-1` —, ou à défaut dans
/// `model.display_name` : voir [`famille_modele`](crate::modele::famille_modele).
/// Un modèle hors table vaut 1, et la ligne se comporte alors comme avant.
///
/// Deux effets, réservés aux fenêtres qui déclarent [`Fenetre::selon_modele`] :
///   - **les seuils s'abaissent** sous un facteur supérieur à 1, de sorte que
///     l'alerte laisse le même temps de réaction. Un facteur inférieur à 1 ne
///     les relève jamais — voir [`adapter_seuil`](crate::modele::adapter_seuil)
///     pour la raison ;
///   - **le rythme se réancre** quand la famille change, la pente mesurée sous
///     l'une ne disant rien de ce que l'autre va consommer — voir
///     `calculer_ancrage` dans [`crate::fenetres`]. Là, c'est la famille qui
///     compte et non la valeur du facteur : Opus 4.8 et Opus 5 partagent la
///     leur.
///
/// Le payload ne porte **aucune** fenêtre propre au modèle, et ce n'est pas une
/// supposition : la capture du 03/09/2026 sur la 2.1.259, en session Fable,
/// ne montre que `five_hour` et `seven_day`. La fenêtre propre à Fable existe
/// pourtant — c'est la « Fable limit » de l'écran `/usage` — et Claude Code la
/// persiste dans `~/.claude.json` : voir [`crate::usage`], qui la lit depuis le
/// soir du 03/09/2026. Le facteur garde son rôle pour la fenêtre **globale**,
/// que tous les modèles remplissent et que Fable remplit deux fois plus vite.
pub(crate) const FACTEURS_MODELE: [(&str, f64); 5] = [
    ("fable", 2.0),
    ("mythos", 2.0),
    ("opus", 1.0),
    ("sonnet", 0.4),
    ("haiku", 0.2),
];

/// Descripteur d'une fenêtre de limitation affichée.
#[derive(Clone, Copy)]
pub(crate) struct Fenetre {
    /// Clé du payload et du cache.
    pub(crate) cle: &'static str,
    /// Préfixe affiché.
    pub(crate) libelle: &'static str,
    pub(crate) seuil: i64,
    pub(crate) seuil_critique: i64,
    /// Réserve l'affichage systématique de l'heure de remise à zéro à la
    /// fenêtre de 5 heures. À l'échelle de la semaine, cette heure n'est
    /// actionnable qu'une fois le plafond en vue :
    /// [`formater_fenetre`](crate::fenetres::formater_fenetre) l'ajoute alors
    /// de lui-même, et la ligne reste courte le reste du temps.
    pub(crate) reset_toujours: bool,
    /// Abaisse les seuils sous un modèle plus cher qu'Opus : la fenêtre est
    /// remplie par tous les modèles, et se vide au prix du jeton. Voir
    /// [`FACTEURS_MODELE`] et [`adapter_seuil`](crate::modele::adapter_seuil).
    pub(crate) seuils_selon_modele: bool,
    /// Réancre le rythme quand la famille du modèle change, et étiquette
    /// l'ancrage de cette famille dans le cache. Voir `calculer_ancrage` dans
    /// [`crate::fenetres`].
    pub(crate) ancrage_selon_modele: bool,
}

/// Fenêtres du payload, dans l'ordre. Cet ordre gouverne aussi celui des clés
/// écrites dans le cache, [`FENETRE_MODELE`] venant après.
///
/// **Seule la fenêtre de 7 jours suit le modèle — 03/09/2026.** C'est elle qui
/// se subit une semaine, et c'est sur elle qu'un modèle deux fois plus cher se
/// paie : son ancrage traverse les sessions, donc les changements de modèle, et
/// sa pente mélangeait jusque-là des jours d'Opus et des heures de Fable. La
/// fenêtre de 5 heures se réancre d'elle-même à chaque remise à zéro, quelques
/// heures au plus après un changement ; la faire taire cinquante minutes à
/// chaque bascule de modèle coûterait plus qu'une pente un peu datée. Ses
/// seuils restent donc ceux d'Opus, et son cache ne porte pas de famille.
pub(crate) const FENETRES: [Fenetre; 2] = [
    Fenetre {
        cle: "five_hour",
        libelle: "5h",
        seuil: SEUIL_FENETRE_5H,
        seuil_critique: SEUIL_CRITIQUE_FENETRE_5H,
        reset_toujours: true,
        seuils_selon_modele: false,
        ancrage_selon_modele: false,
    },
    Fenetre {
        cle: "seven_day",
        libelle: "7j",
        seuil: SEUIL_FENETRE_7J,
        seuil_critique: SEUIL_CRITIQUE_FENETRE_7J,
        reset_toujours: false,
        seuils_selon_modele: true,
        ancrage_selon_modele: true,
    },
];

/// Fenêtre hebdomadaire **propre au modèle**, relevée par `/usage` et lue dans
/// `~/.claude.json` — 03/09/2026, au soir. Voir [`crate::usage`].
///
/// Même libellé que la fenêtre globale de 7 jours : les deux se disputent la
/// même place sur la ligne, et c'est celle-ci qui l'occupe, sauf quand la
/// globale est en alerte et plus remplie — voir `retenir` dans
/// [`crate::fenetres`]. Sur ce poste le 03/09/2026 : 28 % pour Fable contre
/// 14 % pour tous les modèles, et `/usage` dit 28 ; le lendemain, 0 % contre
/// 2 %, et `/usage` dit 0.
///
/// Sa remise à zéro peut être **nulle** : c'est ce que l'API rend tant que le
/// modèle n'a rien consommé depuis la dernière — voir [`crate::usage`]. Le
/// segment se passe alors d'heure et de rythme, jamais de pourcentage.
///
/// Ses seuils sont ceux d'Opus, sans adaptation : ce budget n'est rempli que
/// par le modèle dont il porte le nom, et son pourcentage dit déjà tout du prix
/// du jeton. Son ancrage, lui, suit la famille — une fenêtre Sonnet n'est pas
/// une fenêtre Fable, et la clé de cache est la même pour toutes.
pub(crate) const FENETRE_MODELE: Fenetre = Fenetre {
    cle: "seven_day_modele",
    libelle: "7j",
    seuil: SEUIL_FENETRE_7J,
    seuil_critique: SEUIL_CRITIQUE_FENETRE_7J,
    reset_toujours: false,
    seuils_selon_modele: false,
    ancrage_selon_modele: true,
};

/// Clé de `~/.claude.json` sous laquelle Claude Code persiste la réponse de
/// `/api/oauth/usage`, et genre des entrées de `utilization.limits[]` qui
/// portent une fenêtre propre à un modèle. Relevés dans le bundle de
/// `claude.exe` 2.1.259 et sur le fichier du poste, le 03/09/2026.
pub(crate) const CLE_USAGE_PERSISTE: &str = "cachedUsageUtilization";
pub(crate) const GENRE_FENETRE_MODELE: &str = "weekly_scoped";

/// Genre de l'entrée de `utilization.limits[]` qui porte la fenêtre
/// hebdomadaire de **tous** les modèles — la même que `rate_limits.seven_day`
/// du payload, vue au même instant que la fenêtre du modèle. C'est le point de
/// référence de la dérive — 05/09/2026, voir `estimer_derive` dans
/// [`crate::usage`].
pub(crate) const GENRE_FENETRE_GLOBALE: &str = "weekly_all";

/// Âge au-delà duquel un relevé de `/usage` s'affiche marqué « ~ » et sans
/// rythme, en secondes.
///
/// Une heure, parce que c'est la borne de Claude Code lui-même : passé ce
/// délai il ne relit plus la valeur persistée. La ligne, elle, continue de
/// l'afficher tant que la fenêtre n'a pas expiré — c'est le seul chiffre
/// disponible, et une valeur datée vaut mieux qu'un retour muet à la fenêtre
/// globale, qui se lirait comme une chute de 28 à 14 %. Le relevé n'est
/// rafraîchi qu'à l'ouverture de `/usage`, mesuré sur le poste : le « ~ » dira
/// donc, le plus souvent, qu'il est temps de l'ouvrir.
pub(crate) const AGE_MAX_USAGE_PERSISTE: i64 = 3_600;

/// Marqueurs accolés au modèle. Ils ne signalent que l'état inhabituel : mode
/// rapide actif, réflexion étendue coupée. Voir
/// [`segment_modele`](crate::segments::segment_modele).
pub(crate) const LIBELLE_MODE_RAPIDE: &str = "fast";
pub(crate) const LIBELLE_SANS_REFLEXION: &str = "sans réflexion";

/// Taille ordinaire de la fenêtre de contexte, en jetons. Au-delà, la session
/// tourne en contexte étendu et le marqueur le dit — voir
/// [`marqueur_fenetre`](crate::segments::marqueur_fenetre).
///
/// Ce seuil n'est pas décoratif : `context_window.used_percentage` se calcule
/// sur la fenêtre effective, si bien que le passage au contexte étendu **divise
/// le pourcentage affiché par cinq** sans que rien n'ait été libéré. Sans le
/// marqueur, « ctx 12% » ne dit pas 12 % de quoi, et la chute se lirait comme
/// un compactage.
pub(crate) const FENETRE_CONTEXTE_ORDINAIRE: f64 = 200_000.0;

/// Durée d'observation minimale avant d'oser une projection, en minutes. En
/// deçà, la pente se calcule sur un ou deux points de pourcentage — ils sont
/// entiers dans le cache — et la projection tiendrait de la divination.
pub(crate) const SPAN_MINIMUM_RYTHME: i64 = 10;

/// Facteur d'extrapolation maximal : rapport toléré entre la durée projetée et
/// la durée réellement observée. Au-delà, le rythme se tait.
///
/// Le plancher en minutes traite les deux fenêtres de la même façon, alors
/// qu'elles n'extrapolent pas à la même échelle. Dix minutes suffisent à dire
/// quelque chose de trois heures ; les mêmes dix minutes déroulées sur sept
/// jours multiplient la mesure par mille, et une rafale d'activité y devient un
/// épuisement annoncé pour le lendemain — vu en conditions réelles à 5 % de
/// consommation hebdomadaire.
pub(crate) const RATIO_MAX_EXTRAPOLATION: i64 = 6;

/// Préfixes du segment de rythme : projection à la fin de fenêtre, et heure
/// d'épuisement quand le plafond arrive avant la remise à zéro.
pub(crate) const PREFIXE_PROJECTION: &str = "→";
pub(crate) const PREFIXE_EPUISEMENT: &str = "épuisé";

/// Marqueurs accolés au pourcentage d'une fenêtre, à gauche de la valeur,
/// quand elle n'est pas une mesure fraîche. Ils qualifient la mesure, pas le
/// compteur, d'où leur place contre le nombre.
///
/// « ~ » : valeur **mémorisée** — le cache de la ligne avant la première
/// réponse de la session, ou un relevé de `/usage` vieux de plus d'une heure.
/// Ancienne, pas fausse.
///
/// « ≈ » — 05/09/2026 : valeur **estimée**. Le relevé de `/usage` ne bouge
/// qu'à l'ouverture de l'écran, mais la fenêtre globale du payload, elle, est
/// fraîche à chaque réponse ; la fenêtre du modèle est alors avancée d'autant,
/// au rapport des deux budgets — voir `estimer_derive` dans [`crate::usage`].
/// Ce n'est plus une mesure, et le marqueur le dit ; le rythme se tait, comme
/// sous « ~ ». U+2248, présent dans Consolas comme le « → » de la projection.
pub(crate) const MARQUEUR_MEMORISEE: &str = "~";
pub(crate) const MARQUEUR_ESTIMEE: &str = "≈";

/// Unité des quatre compteurs — contexte, deux fenêtres, projection.
///
/// Elle est passée du côté du chrome le 25/08/2026, et le littéral est ici pour
/// que ce déplacement n'ait qu'un seul point d'entrée : voir
/// [`formater_mesure`](crate::sortie::formater_mesure).
pub(crate) const UNITE_POURCENT: &str = "%";

// ---------------------------------------------------------------------------
// Palette de la capsule — 19/09/2026
// ---------------------------------------------------------------------------
//
// Tout est en couleurs vraies (`38;2` / `48;2`). La ligne l'était déjà pour la
// teinte de marque ; la palette 256, qui servait à « tenir sur n'importe quel
// fond de thème », n'a plus lieu d'être : chaque teinte d'avant-plan est réglée
// pour le fond du compartiment où elle sert, connu et choisi. Les rapports de
// contraste cités sont WCAG 2, calculés contre ce fond-là — le thème du
// terminal n'entre plus que dans un seul rapport, celui du corps contre son
// fond (1,7:1 sur Dark+), qui dit si la forme se voit.
//
// Les teintes de fond sont des triplets `r;g;b` nus, sans préfixe SGR : la
// même sert de fond au compartiment (`48;2;…`) et d'encre aux caps et à la
// jonction qui le prolongent (`38;2;…`). Les avant-plans sont des séquences
// complètes, comme avant.
//
// D'où viennent ces valeurs, et ce qu'elles ont remplacé : `statusline-rust.md`
// §23. En un mot : le vert 22/157 de la pastille d'emplacement, le gris 245 du
// chrome, l'ambre 214, le rouge 203 et le cyan 109 ont été réaccordés autour
// de la teinte de marque, et le fond `#300E00` de la pastille de tête — 1,06:1
// contre le fond Dark+, invisible à l'écran — a cédé la place à un aplat de
// cette teinte.

/// Teinte de la **tête** : `rgb(215,119,87)`, la couleur du corps de `clawd`
/// dans le bundle de `claude.exe` — nommée `clawd_body` par ses six thèmes en
/// couleurs vraies, clair, sombre et daltonisés confondus. Ce n'est pas une
/// couleur de thème mais une couleur de marque, et elle ne bouge pas.
///
/// Elle teinte le **fond** du compartiment depuis la capsule, là où elle
/// teintait le texte de la pastille sur un fond `#300E00` presque noir : ce fond
/// faisait 1,06:1 contre le `#1E1E1E` de Dark+ et n'existait pas à l'écran, la
/// « pastille » était un mot orange nu. Inversée, la marque devient une forme,
/// et le texte brun qui la porte tient 5,7:1.
pub(crate) const RVB_TETE: &str = "215;119;87";

/// Teinte du **corps** : une ardoise `#3b414b`, fond de l'identité et des
/// mesures tant qu'aucun palier n'est atteint. Bleutée pour s'opposer à la
/// chaleur de la marque ; 1,7:1 contre le fond Dark+, assez pour que la forme
/// se voie sous l'acrylic, pas assez pour crier.
pub(crate) const RVB_CORPS: &str = "59;65;75";

/// Teinte de l'**emplacement** : un vert sauge profond `#274a31`, héritier de
/// la pastille 22/157 qui, du 21/08 au 19/09/2026, a été la seule ancre de
/// couleur de la ligne — sur un workspace multi-projets, savoir d'où l'on
/// parle prime sur tout le reste. Désaturé vers la chaleur du reste de la
/// palette ; le texte clair y tient 7,4:1, le sourd 4,9:1.
pub(crate) const RVB_LIEU: &str = "39;74;49";

/// Teinte du compartiment des mesures au palier d'**alerte** : un ambre sombre
/// `#4a3d1c`, sur lequel l'ambre des valeurs tient 5,9:1 et le chrome 4,8:1.
pub(crate) const RVB_ALERTE: &str = "74;61;28";

/// Teinte du compartiment des mesures au palier **critique** : un rouge sombre
/// `#532825`, sur lequel le corail des valeurs tient 5,6:1 et le chrome 5,5:1.
pub(crate) const RVB_CRITIQUE: &str = "83;40;37";

/// Séquence SGR du texte **plein** : le nom du modèle, une valeur sous ses
/// seuils, le plein d'une jauge. `#eceef1`, 11,7:1 sur l'ardoise.
///
/// Explicite depuis la capsule : jusque-là ces fragments sortaient sans
/// séquence, dans la couleur par défaut du terminal, réglée pour le fond du
/// thème et non pour celui d'un compartiment.
pub(crate) const CODE_TEXTE: &str = "38;2;236;238;241";

/// Séquence SGR du texte de la **tête**, brun sur la teinte de marque :
/// `#2a0d02`, 5,7:1 — voir [`RVB_TETE`].
pub(crate) const CODE_TEXTE_TETE: &str = "38;2;42;13;2";

/// Séquence SGR de la feuille du chemin et de la branche, en clair sur la
/// sauge : `#c6e8b8`, 7,4:1 — ce qui distingue le sous-projet.
pub(crate) const CODE_TEXTE_LIEU: &str = "38;2;198;232;184";

/// Séquence SGR des ancêtres du chemin et des parenthèses de la branche, en
/// sourd sur la sauge : `#93c08e`, 4,9:1 — ce qui situe. Le chemin est
/// hiérarchisé depuis la capsule : l'œil tombe sur la feuille, le reste se
/// lit s'il le faut.
pub(crate) const CODE_TEXTE_LIEU_SOURD: &str = "38;2;147;192;142";

/// Séquences SGR des deux paliers, réglées sur les fonds de palier.
///
/// L'ambre `#f2ab3f` remplace le 214 (`#ffaf00`) du 25/08/2026 : moins saturé,
/// plus proche du corail de la marque, 5,9:1 sur l'ambre sombre. Le corail
/// `#f9948a` remplace le 203 (`#ff5f5f`) : éclairci pour tenir 5,6:1 sur le
/// rouge sombre, là où le 203 n'aurait fait que 3,6:1.
///
/// Le gras du palier critique **n'est plus dans la séquence** : c'était un
/// `1;` de tête que le `ESC[0m` final défaisait avec le reste. Sans remise à
/// zéro, chaque propriété referme la sienne — voir `gras` dans
/// [`crate::sortie`]. Il reste un signal, la dernière marche avant le plafond,
/// où l'épaisseur double la couleur.
pub(crate) const CODE_ALERTE: &str = "38;2;242;171;63";
pub(crate) const CODE_CRITIQUE: &str = "38;2;249;148;138";

/// Séquence SGR des marqueurs de session — 25/08/2026, réaccordée le
/// 19/09/2026.
///
/// `1M`, `fast` et `sans réflexion` sortaient dans la couleur pleine du thème,
/// celle du nom du modèle. Ils se lisaient donc comme une partie de ce nom —
/// « Opus 5 1M » — alors qu'ils décrivent un **état de session** et ne
/// paraissent que sur l'inhabituel. Un bleu-gris `#93c5d0`, 6,0:1 sur
/// l'ardoise : plus lumineux que le chrome, moins que le nom, ce qui est
/// exactement leur rang, et complémentaire de la marque plutôt que du vert de
/// l'emplacement. Le cyan 109 qui le précédait avait été choisi contre la
/// pastille verte.
///
/// L'effort, lui, garde son gris : il qualifie le modèle et se consulte, il
/// n'annonce pas un écart.
pub(crate) const CODE_MARQUEUR: &str = "38;2;147;197;208";

/// Séquence SGR des libellés, unités et heures : un gris légèrement froid
/// `#a9b0b8`, 4,9:1 sur l'ardoise, 4,8:1 et 5,5:1 sur les deux fonds de
/// palier. Il remplace le 245 de la palette 256, qui avait été choisi pour
/// tenir sur fond sombre comme clair ; il n'a plus qu'un fond à tenir.
pub(crate) const CODE_ATTENUE: &str = "38;2;169;176;184";

/// Rainures de la jauge fine, une par fond de palier — 24/09/2026, chantier
/// « mesures », piste C.
///
/// La rainure est le **fond** des deux cellules de la jauge : le huitième de
/// bloc y trace le plein dans l'encre du palier, le reste de la cellule laisse
/// voir la rainure. Un ton au-dessus du fond du segment — 1,46:1 sur
/// l'ardoise, 1,58:1 sur l'ambre sombre, 1,44:1 sur le rouge sombre —, et le
/// plein y tient 6,1:1 en texte plein, 3,4:1 en ambre, 4,0:1 en corail,
/// au-dessus du 3:1 d'un élément graphique.
///
/// Celle de l'ardoise est l'ancienne **piste** `#525965`, l'encre du `░` vide
/// de la jauge à sept crans (`CODE_PISTE`, du 19/09 au 24/09/2026), passée de
/// l'avant-plan au fond. Le §23 du dossier se demandait si
/// `adjustIndistinguishableColors: "always"` relèverait ce gris à 1,5:1 de
/// l'ardoise ; d'après sa description, le réglage ne touche qu'aux
/// avant-plans, et un fond lui échappe.
///
/// Claire plutôt que sombre, sur comparaison à l'écran : une rainure sombre
/// (`41;45;52`) prenait presque la teinte du fond du terminal sous l'acrylique
/// — relevé entre `37,37,36` et `56,53,49` sur la même capture — et se lisait
/// comme une fente dans la capsule, changeante avec ce qui passe derrière la
/// fenêtre. La claire se lit comme une piste, quel que soit l'arrière-plan.
pub(crate) const RVB_RAINURE_CORPS: &str = "82;89;101";
pub(crate) const RVB_RAINURE_ALERTE: &str = "106;90;48";
pub(crate) const RVB_RAINURE_CRITIQUE: &str = "112;62;58";

// Les deux pastilles — `CODE_PASTILLE`, vert 22 sur 157, du 21/08 au 19/09/2026,
// et `CODE_PASTILLE_ABONNEMENT`, teinte de marque sur `#300E00`, du 26/08 au
// 19/09/2026 — ont été les seuls fonds de la ligne avant la capsule. Leur
// histoire, leurs arbitrages de contraste et les trois coûts qu'elles
// prévoyaient à un fond courant — que la capsule paie autrement — sont au
// dossier `statusline-rust.md`, §13, §15, §17 et §23.

/// Largeur du champ de valeur, unité comprise, cadré à droite. Sans ce calage,
/// la ligne se décale à chaque rafraîchissement : « ctx 9% » devenant
/// « ctx 10% », tout ce qui suit glisse d'un caractère, et l'œil doit retrouver
/// des repères qui n'ont pourtant pas changé de nature.
///
/// **Étalonnée sur « 99% » depuis le 25/08/2026**, contre « 100% » auparavant.
/// Le plafond n'arrive qu'une fois par fenêtre ; le reste du temps, ce quatrième
/// caractère était un blanc, et il se cumulait avec l'espace de séparation en un
/// trou de deux colonnes sur chacun des trois compteurs — « ctx  34% ».
///
/// Le prix est un débordement d'un caractère à 100 %, exactement celui déjà
/// accepté pour le « ~100% » d'une valeur mémorisée : perdre l'alignement une
/// requête durant coûte moins cher que le trou permanent.
pub(crate) const LARGEUR_VALEUR: usize = 3;

/// Crans de la micro-jauge, du plus vide au plus plein — **deux cellules depuis
/// le 26/08/2026**, second lot de retouches.
///
/// **Sous `NO_COLOR` seulement depuis le 24/09/2026.** En couleur, c'est la
/// jauge fine de [`HUITIEMES_JAUGE`] qui sert, seize crans sur une rainure ;
/// ici la ligne reste, octet pour octet, celle que le harnais compare à
/// l'oracle. Ce qui suit reste vrai de cette table, et dit pourquoi elle
/// comptait sept crans.
///
/// Un nombre se lit, un remplissage se voit. La jauge donne l'état d'une fenêtre
/// avant que l'œil n'atteigne le chiffre, pour deux colonnes — là où une jauge à
/// cinq caractères en coûterait cinq sans rien apprendre que le nombre ne dise.
///
/// # Sept crans, et non huit
///
/// La cellule de gauche se remplit d'abord, celle de droite ensuite : c'est la
/// sémantique d'une jauge, et elle se lit sans qu'on ait à comparer deux
/// densités entre elles. Avec quatre densités par cellule, la chaîne
/// **strictement croissante** que cela autorise compte sept crans et pas un de
/// plus — `░░ ▒░ ▓░ █░ █▒ █▓ ██` — puisqu'un cran comme `▒▒` ne se range ni
/// avant ni après `█░` sans arbitraire. La proposition en annonçait huit ; c'est
/// l'échelle qui tranche, pas le vœu. Un cran vaut donc 14,3 points, contre 25
/// avec une cellule unique.
///
/// Les huit crans de hauteur (`▁▂▃▄▅▆▇█`) les donneraient d'un seul caractère,
/// mais six d'entre eux sont absents de Consolas — c'est le constat du matin même
/// (voir plus bas), et il n'a pas changé.
///
/// **La jauge était en hauteur — `▁▂▃▄▅▆▇█` — jusqu'au 25/08/2026**, sur la foi
/// d'un commentaire affirmant que Consolas possédait le bloc « Block Elements »
/// en entier. La table de la police dit le contraire : `▄` et `█` y sont, les
/// six autres n'y sont pas. Le terminal les faisait donc dessiner par une police
/// de repli, qui ignore le corps 8 *light* demandé et son propre alignement
/// vertical : la jauge changeait de main au milieu de sa propre échelle, entre
/// `▃` et `▄`.
///
/// D'où la densité : `░▒▓█` sont tous les quatre dans Consolas. La densité dit
/// d'ailleurs « plein » plus directement que la hauteur, qui se compare mal d'un
/// caractère isolé à un autre.
///
/// Le nombre de crans n'est écrit nulle part ailleurs :
/// [`bloc_jauge`](crate::sortie) découpe les cent points sur la longueur de
/// cette table, quelle que soit la largeur des crans. Passer de quatre à sept
/// n'a donc rien demandé d'autre que cette table — et une jauge à seize crans
/// resterait hors d'atteinte sans changer de police, les blocs partiels
/// `▏▎▍▌▋▊▉` étant absents de Consolas comme les blocs de hauteur. Le
/// 24/09/2026 l'a démenti pour ce terminal-ci : Windows Terminal trace ces
/// blocs lui-même, sans les demander à la police — voir [`HUITIEMES_JAUGE`].
///
/// La jauge est réservée aux **fenêtres de limitation** et ne paraît pas sur le
/// contexte. La distinction est de fond : une fenêtre se remplit vers un
/// plafond qu'on subit, alors que le contexte se compacte — lui donner une
/// jauge suggérerait une fatalité qu'il n'a pas.
pub(crate) const BLOCS_JAUGE: [&str; 7] = ["░░", "▒░", "▓░", "█░", "█▒", "█▓", "██"];

/// Huitièmes de la jauge fine, du vide au plein — 24/09/2026, chantier
/// « mesures », piste C : le blanc de la cellule vide, puis `▏ ▎ ▍ ▌ ▋ ▊ ▉ █`
/// (`U+258F` à `U+2588`).
///
/// Deux cellules, seize crans : la cellule de gauche se remplit d'abord,
/// huitième par huitième, puis celle de droite — la sémantique de
/// [`BLOCS_JAUGE`], à une résolution plus que doublée, 6,25 points par cran
/// contre 14,3. Le vide n'est pas un glyphe mais la **rainure**, un fond de
/// cellule — voir [`RVB_RAINURE_CORPS`].
///
/// Ces glyphes ne sont pas dans Consolas, et c'est ce qui avait suspendu la
/// jauge à seize crans le 26/08/2026. Windows Terminal 1.24 les trace
/// lui-même, comme les caps Powerline : relevé au pixel le 24/09/2026 sur une
/// capture en corps 8, huit largeurs distinctes — 1, 2, 3, 5, 6, 7, 8 et 9
/// pixels sur une cellule de 9. Un autre terminal les demanderait à une police
/// de repli ; celui-ci est le seul que la ligne vise.
///
/// En couleur seulement : sans rainure, une jauge à 10 % ne serait qu'un trait
/// isolé. Sous `NO_COLOR`, c'est [`BLOCS_JAUGE`] qui sert. La jauge reste
/// réservée aux fenêtres, pour la raison donnée plus haut : le chantier l'a
/// proposée pour le contexte, et l'utilisateur a maintenu la règle.
pub(crate) const HUITIEMES_JAUGE: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

/// Nombre de segments de chemin affichés avant repli en « …\feuille » —
/// « racine\…\feuille » jusqu'au 23/09/2026.
///
/// Deux, et non plus trois, depuis le même jour : l'utilisateur tient déjà
/// « PY_xl\.claude\user-config » pour un chemin long. La racine suivie d'un
/// seul sous-dossier reste donc entière, tout ce qui descend plus bas se
/// réduit à sa feuille.
pub(crate) const PROFONDEUR_MAX_CHEMIN: usize = 2;

/// Remontée maximale à la recherche d'un « .git ». Un lien mal formé ne peut
/// ainsi pas faire tourner la boucle indéfiniment, et 32 niveaux dépassent
/// toute arborescence réelle.
pub(crate) const REMONTEE_MAX_GIT: usize = 32;

/// Fichier volatile, sous `%LOCALAPPDATA%` et non à côté du binaire : la copie
/// versionnée est sur un lecteur Google Drive, où un fichier réécrit à chaque
/// changement de pourcentage n'a rien à faire.
pub(crate) const DOSSIER_ETAT: &str = "claude-code";
pub(crate) const NOM_CACHE: &str = "statusline-cache.json";

/// Tentatives de lecture du cache, et attente entre deux, en millisecondes.
///
/// Une lecture peut échouer sans que le fichier soit en cause : accès concurrent
/// pendant le remplacement atomique mené par une autre instance, ou blocage
/// passager du disque. Le prix de cet échec est disproportionné — sans ancrage,
/// la pente ne se calcule plus et le segment de rythme se retire pour ce seul
/// lancement, que le rafraîchissement d'une seconde ramène aussitôt. L'œil y
/// voit un clignotement, mesuré le 23/08/2026 à deux tirs sur 946, les deux
/// étant aussi les plus lents du relevé.
///
/// Deux réessais brefs ferment cette classe entière sans rien coûter en régime
/// normal : la première lecture réussit dans plus de 99 % des cas, et un cache
/// absent — le premier lancement d'une session — n'attend jamais, l'erreur
/// « fichier introuvable » étant tenue pour définitive.
pub(crate) const TENTATIVES_LECTURE_CACHE: u32 = 3;
pub(crate) const ATTENTE_RELECTURE_CACHE_MS: u64 = 5;

/// Journal de diagnostic, et le témoin qui l'active. Voir [`crate::journal`].
pub(crate) const NOM_JOURNAL: &str = "statusline-journal.jsonl";
pub(crate) const NOM_TEMOIN_JOURNAL: &str = "statusline-journal.on";

/// Capture du payload, et le témoin qui l'arme. Voir [`crate::capture`].
///
/// Mêmes conventions que le journal — un fichier témoin posé à côté du fichier
/// produit — à une différence près : ce témoin-ci est **consommé** par la
/// capture, qui ne relève qu'un payload et se rendort. Un rafraîchissement par
/// seconde réécrirait sinon le fichier indéfiniment.
pub(crate) const NOM_CAPTURE: &str = "statusline-payload.json";
pub(crate) const NOM_TEMOIN_CAPTURE: &str = "statusline-payload.on";
