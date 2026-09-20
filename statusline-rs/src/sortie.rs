//! Sortie : écriture de la ligne, coloration, mise en forme d'une mesure — et,
//! depuis le 19/09/2026, la **capsule** qui enveloppe chaque ligne.
//!
//! Section 2 du script d'origine, augmentée de [`formater_mesure`], qui vivait
//! en tête de la section 6, puis de [`encapsuler`] et [`assembler_capsules`],
//! qui n'ont pas d'équivalent dans l'oracle : à partir de la capsule, le script
//! PowerShell ne décrit plus que la ligne sous `NO_COLOR`.
//!
//! # Le modèle de rendu depuis la capsule
//!
//! Jusqu'au 19/09/2026, chaque fragment coloré se refermait par `ESC[0m`, et la
//! règle était « fragments juxtaposés, jamais imbriqués » : un fond ne pouvait
//! couvrir qu'un fragment à la fois, d'où les deux pastilles. La capsule pose
//! un fond sur toute une ligne, et la règle a changé avec elle :
//!
//! 1. **le fond appartient au compartiment.** Il est posé une fois, en tête, et
//!    reste ouvert jusqu'au compartiment suivant. Les fragments ne touchent
//!    qu'à l'avant-plan et au gras, et ne referment que ce qu'ils ouvrent :
//!    `ESC[22m` après un gras, rien après une couleur puisque le fragment
//!    suivant pose la sienne. La ligne ne porte que deux `ESC[0m`, autour du
//!    cap droit ;
//! 2. **les couleurs sont réglées pour le fond du compartiment**, connu et
//!    choisi, et plus pour un fond de thème inconnu. Le texte « plein » n'est
//!    donc plus la couleur du terminal mais une constante, [`CODE_TEXTE`] ;
//! 3. **le corps est neutre, c'est le palier qui colore** : le compartiment des
//!    mesures prend le fond du pire palier qu'il porte — voir
//!    [`teinte_du_palier`].
//!
//! Sous `NO_COLOR`, tout ce qui n'est que forme se retire — fonds, caps,
//! jonctions, liserés, saut de ligne — et ce qui sort est, octet pour octet, la
//! ligne d'avant la capsule. C'est ce qui fait des cas non colorés du harnais
//! un filet de non-régression sans qu'on y touche.

use std::io::Write;

use crate::reglages::{
    BLOCS_JAUGE, BORD_BAS, BORD_HAUT, CAP_DROIT, CAP_GAUCHE, CODE_ALERTE, CODE_ATTENUE,
    CODE_CRITIQUE, CODE_MARQUEUR, CODE_PISTE, CODE_SEPARATEUR, CODE_TEXTE, JONCTION,
    LARGEUR_VALEUR, LISERE, MODELE_PAR_DEFAUT, RVB_ALERTE, RVB_CONTOUR, RVB_CORPS, RVB_CRITIQUE,
    SEPARATEUR,
};

/// Écrit la ligne sur la sortie standard, en octets UTF-8.
///
/// Le script d'origine devait court-circuiter deux couches de PowerShell — la
/// page de codes OEM et `$PSStyle.OutputRendering`, qui abîmaient
/// respectivement le séparateur « · » et les séquences ANSI. Ici il ne reste
/// que l'écriture des octets. Depuis la capsule, « la ligne » peut en faire
/// deux, séparées d'un `\n` que [`assembler_capsules`] a posé : Claude Code
/// affiche toutes les lignes non vides que la commande écrit.
pub(crate) fn ecrire_ligne(ligne: &str) {
    let mut sortie = std::io::stdout();
    let _ = sortie.write_all(ligne.as_bytes());
    let _ = sortie.write_all(b"\n");
    let _ = sortie.flush();
}

/// Rend la ligne telle quelle si elle porte quelque chose, sinon un repli.
///
/// Une ligne blanche n'est pas un affichage minimal, c'est un effacement :
/// Claude Code découpe la sortie, retire les lignes vides, et un résultat vide
/// remplace le texte affiché par rien. Tous les segments pouvant se retirer
/// d'eux-mêmes, la ligne assemblée passe par ce filtre — et le repli lui-même,
/// qui vient du payload, est vérifié à son tour.
pub(crate) fn confirmer_ligne(ligne: &str, repli: &str) -> String {
    if !ligne.trim().is_empty() {
        return ligne.to_string();
    }
    if !repli.trim().is_empty() {
        return repli.to_string();
    }
    MODELE_PAR_DEFAUT.to_string()
}

/// Indique si la coloration est désactivée, selon la convention `NO_COLOR`.
///
/// La véracité de PowerShell est reproduite telle quelle : une variable définie
/// mais vide ne désactive rien, alors que `NO_COLOR=0` désactive — c'est une
/// chaîne non vide, donc vraie.
///
/// Lue **une fois par lancement** depuis le 04/09/2026 : chaque fragment coloré
/// passait par une lecture de l'environnement, soit quelques dizaines par
/// ligne, pour une valeur qui ne change pas en cours de processus. La lecture
/// se fait en `OsString`, si bien qu'une valeur qui ne serait pas de l'UTF-8
/// valide compte comme définie et non vide — ce que PowerShell en ferait
/// aussi.
pub(crate) fn sans_couleur() -> bool {
    static SANS_COULEUR: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SANS_COULEUR.get_or_init(|| {
        std::env::var_os("NO_COLOR")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    })
}

// ---------------------------------------------------------------------------
// Paliers
// ---------------------------------------------------------------------------

/// Palier atteint par une mesure : aucun, alerte, critique — dans cet ordre,
/// pour que le pire de plusieurs paliers soit leur maximum.
///
/// Il était implicite jusqu'au 19/09/2026, chaque compteur comparant lui-même
/// son pourcentage à ses deux seuils au moment de se colorer. La capsule a
/// besoin de le **remonter** : le compartiment des mesures prend le fond du
/// pire palier qu'il porte, et c'est l'assemblage qui pose ce fond, pas le
/// compteur.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Palier {
    Aucun,
    Alerte,
    Critique,
}

/// Rend le palier d'un pourcentage.
///
/// Les deux seuils sont testés du plus haut au plus bas, si bien qu'un seuil
/// critique mal réglé — inférieur au seuil d'alerte — dégrade l'affichage vers
/// le rouge plutôt que de rendre l'un des deux paliers inatteignable.
pub(crate) fn palier(pourcent: i64, seuil: i64, seuil_critique: i64) -> Palier {
    if pourcent >= seuil_critique {
        Palier::Critique
    } else if pourcent >= seuil {
        Palier::Alerte
    } else {
        Palier::Aucun
    }
}

/// Rend la teinte de fond du compartiment des mesures pour un palier : le
/// corps neutre, ou l'ambre sombre, ou le rouge sombre.
///
/// C'est la troisième règle de la capsule : un fond permanent dirait « tout va
/// bien » ; celui-ci ne dit rien tant que rien ne s'écarte de l'ordinaire, et
/// se teinte exactement quand les valeurs se teintent.
pub(crate) fn teinte_du_palier(palier: Palier) -> &'static str {
    match palier {
        Palier::Aucun => RVB_CORPS,
        Palier::Alerte => RVB_ALERTE,
        Palier::Critique => RVB_CRITIQUE,
    }
}

/// Séquence d'avant-plan d'un palier : texte plein, ambre, ou corail.
fn encre_du_palier(palier: Palier) -> &'static str {
    match palier {
        Palier::Aucun => CODE_TEXTE,
        Palier::Alerte => CODE_ALERTE,
        Palier::Critique => CODE_CRITIQUE,
    }
}

// ---------------------------------------------------------------------------
// Coloration
// ---------------------------------------------------------------------------

/// Ouvre une séquence ANSI de couleur devant un fragment — **sans la
/// refermer**.
///
/// Jusqu'au 19/09/2026 chaque fragment se refermait par `ESC[0m`. Ce n'est plus
/// possible dans une capsule : la remise à zéro emporterait le fond du
/// compartiment. Le fragment suivant pose sa propre couleur, un blanc hérite
/// de la précédente sans qu'on le voie, et c'est [`encapsuler`] qui referme,
/// une fois, en fin de ligne.
pub(crate) fn colorer(texte: &str, code: &str) -> String {
    colorer_selon(texte, code, sans_couleur())
}

/// Cœur de [`colorer`], la décision passée en paramètre plutôt que lue dans
/// l'environnement : les deux branches deviennent ainsi testables sans toucher
/// à `NO_COLOR`, qu'un test modifierait pour tous les autres — et qui se trouve
/// justement défini dans l'environnement des processus lancés par Claude Code,
/// `cargo test` compris.
fn colorer_selon(texte: &str, code: &str, sans_couleur: bool) -> String {
    if sans_couleur {
        return texte.to_string();
    }
    format!("\u{1b}[{}m{}", code, texte)
}

/// Met un fragment en gras, et referme le gras — `ESC[1m` … `ESC[22m`.
///
/// Brique à part depuis le 19/09/2026 : le gras vivait dans `CODE_CRITIQUE`,
/// sous la forme d'un `1;` de tête que le `ESC[0m` final défaisait avec le
/// reste. Sans remise à zéro, chaque propriété doit refermer la sienne, et le
/// `22` ne refermerait pas ce qu'un `1;` a ouvert au milieu d'une séquence de
/// couleur. Une séquence par propriété, donc.
///
/// Son seul appelant est [`au_palier_selon`], qui tient déjà la décision de
/// coloration : elle est donc passée en paramètre, sans le doublon lisant
/// l'environnement que portent [`colorer`] et ses semblables.
fn gras_selon(texte: &str, sans_couleur: bool) -> String {
    if sans_couleur {
        return texte.to_string();
    }
    format!("\u{1b}[1m{}\u{1b}[22m", texte)
}

/// Pose un fragment en texte **plein** : le nom du modèle, une valeur sous ses
/// seuils.
///
/// Explicite depuis la capsule : la couleur par défaut du terminal était
/// réglée pour son fond, pas pour l'ardoise du compartiment. Voir
/// [`CODE_TEXTE`].
pub(crate) fn texte_plein(texte: &str) -> String {
    colorer(texte, CODE_TEXTE)
}

/// Atténue un fragment de chrome : libellé, unité, ponctuation ne portant
/// aucune valeur.
pub(crate) fn attenuer(texte: &str) -> String {
    colorer(texte, CODE_ATTENUE)
}

/// Pose un marqueur de session : contexte étendu, mode rapide, réflexion
/// coupée.
///
/// Ces trois-là ne sont ni du chrome ni une mesure. Ils ne paraissent que sur
/// l'état inhabituel, et leur teinte propre les détache du nom du modèle auquel
/// ils étaient jusqu'ici indistinguables. Voir [`CODE_MARQUEUR`].
pub(crate) fn marquer(texte: &str) -> String {
    colorer(texte, CODE_MARQUEUR)
}

/// Rend le séparateur de segment, prêt à être posé entre deux segments d'un
/// même compartiment.
///
/// Un séparateur structure, il n'informe pas, et n'a donc rien à faire dans la
/// couleur pleine. Voir [`CODE_SEPARATEUR`]. Depuis la capsule, c'est le seul
/// séparateur qui s'écrive : le rang supérieur — entre groupes — est devenu la
/// jonction de deux compartiments, et le rang inférieur — entre morceaux d'un
/// segment — reste un espace.
pub(crate) fn separateur() -> String {
    colorer(SEPARATEUR, CODE_SEPARATEUR)
}

/// Colore une valeur selon son palier : texte plein, ambre, ou corail **en
/// gras** — le gras reste le signal de la dernière marche avant le plafond, où
/// l'épaisseur double la couleur.
pub(crate) fn au_palier(texte: &str, palier: Palier) -> String {
    au_palier_selon(texte, palier, sans_couleur())
}

/// Cœur de [`au_palier`], la décision de coloration passée en paramètre.
fn au_palier_selon(texte: &str, palier: Palier, sans_couleur: bool) -> String {
    let colore = colorer_selon(texte, encre_du_palier(palier), sans_couleur);
    match palier {
        Palier::Critique => gras_selon(&colore, sans_couleur),
        _ => colore,
    }
}

// ---------------------------------------------------------------------------
// Jauge et mesures
// ---------------------------------------------------------------------------

/// Choisit le cran de jauge correspondant à un pourcentage.
///
/// Les cent points sont découpés sur la longueur de [`BLOCS_JAUGE`], quelle
/// qu'elle soit : sept crans sur deux cellules aujourd'hui, donc un cran tous
/// les 14,3 %. Les valeurs hors bornes se rabattent sur les extrémités plutôt
/// que d'échouer — un pourcentage négatif ou supérieur à cent ne devrait pas
/// exister, mais la ligne ne se perd jamais sur une valeur inattendue.
fn bloc_jauge(pourcent: i64) -> &'static str {
    let index = (pourcent.clamp(0, 100) * BLOCS_JAUGE.len() as i64 / 100)
        .min(BLOCS_JAUGE.len() as i64 - 1) as usize;
    BLOCS_JAUGE[index]
}

/// Colore un cran de jauge **cellule par cellule** : le vide `░` en piste, le
/// reste dans l'encre du palier.
///
/// Jusqu'au 19/09/2026 le cran entier prenait la couleur du palier, si bien
/// que sous le seuil le vide était aussi lumineux que le plein et que la
/// densité se lisait moins qu'elle ne le pouvait. La piste ne porte pas de
/// gras au palier critique : l'épaisseur est un signal pour un nombre, pas
/// pour un aplat.
fn colorer_jauge(cran: &str, palier: Palier) -> String {
    colorer_jauge_selon(cran, palier, sans_couleur())
}

/// Cœur de [`colorer_jauge`], la décision de coloration passée en paramètre.
fn colorer_jauge_selon(cran: &str, palier: Palier, sans_couleur: bool) -> String {
    cran.chars()
        .map(|cellule| {
            let code = if cellule == '░' {
                CODE_PISTE
            } else {
                encre_du_palier(palier)
            };
            colorer_selon(&cellule.to_string(), code, sans_couleur)
        })
        .collect()
}

/// Met en forme un triplet libellé/valeur/unité : libellé et unité atténués,
/// valeur cadrée à droite et colorée selon le palier atteint — rendu avec la
/// mesure, pour que l'assemblage teinte le compartiment.
///
/// C'est la brique de la hiérarchie visuelle de la ligne. Tous les compteurs
/// passent par elle, ce qui garantit qu'ils se lisent de la même façon : le
/// libellé situe, la valeur porte l'information et se voit sans être cherchée.
///
/// **L'unité a rejoint le chrome le 25/08/2026.** Le « % » ne varie jamais et
/// ne porte aucune information ; il occupait pourtant trois colonnes en pleine
/// couleur d'alerte, et le gras du palier critique s'appliquait à lui comme aux
/// chiffres.
pub(crate) fn formater_mesure(
    libelle: &str,
    valeur: &str,
    unite: &str,
    pourcent: i64,
    seuil: i64,
    seuil_critique: i64,
) -> (String, Palier) {
    formater_mesure_ornee(
        libelle,
        false,
        valeur,
        unite,
        pourcent,
        seuil,
        seuil_critique,
    )
}

/// Comme [`formater_mesure`], en glissant la micro-jauge entre le libellé et la
/// valeur : `5h ▒░ 20%`.
///
/// La jauge prend la **couleur du palier**, comme la valeur qu'elle annonce.
/// C'est ce qui lui évite d'introduire un vert permanent : la ligne ne dit
/// jamais « tout va bien », elle se tait quand tout va bien.
///
/// Réservée aux fenêtres de limitation, pour la raison exposée en tête de
/// [`BLOCS_JAUGE`].
pub(crate) fn formater_mesure_jaugee(
    libelle: &str,
    valeur: &str,
    unite: &str,
    pourcent: i64,
    seuil: i64,
    seuil_critique: i64,
) -> (String, Palier) {
    formater_mesure_ornee(
        libelle,
        true,
        valeur,
        unite,
        pourcent,
        seuil,
        seuil_critique,
    )
}

/// Cœur des deux précédentes.
///
/// Les blancs — séparateurs et calage — restent hors des séquences : ils
/// héritent du fond du compartiment, et n'ont pas de couleur d'avant-plan à
/// porter.
///
/// La largeur se compte en caractères, non en octets : « épuisé », les chemins
/// accentués et les blocs de jauge se décaleraient sinon.
///
/// Le calage porte sur la valeur **et son unité**, alors que la coloration les
/// sépare : ce qui doit rester stable d'un rafraîchissement à l'autre est la
/// colonne où finit le compteur, pas celle où finissent ses chiffres.
fn formater_mesure_ornee(
    libelle: &str,
    jauger: bool,
    valeur: &str,
    unite: &str,
    pourcent: i64,
    seuil: i64,
    seuil_critique: i64,
) -> (String, Palier) {
    let palier = palier(pourcent, seuil, seuil_critique);

    let largeur = valeur.chars().count() + unite.chars().count();
    let calage = " ".repeat(LARGEUR_VALEUR.saturating_sub(largeur));

    let jauge = if jauger {
        format!("{} ", colorer_jauge(bloc_jauge(pourcent), palier))
    } else {
        String::new()
    };

    // Une unité vide ne produit pas une séquence de couleur vide : elle
    // n'afficherait rien tout en pesant sur la ligne, et se verrait sur toute
    // comparaison octet à octet.
    let suffixe = if unite.is_empty() {
        String::new()
    } else {
        attenuer(unite)
    };

    (
        format!(
            "{} {}{}{}{}",
            attenuer(libelle),
            jauge,
            calage,
            au_palier(valeur, palier),
            suffixe
        ),
        palier,
    )
}

// ---------------------------------------------------------------------------
// Capsule
// ---------------------------------------------------------------------------

/// Un compartiment de la capsule : une teinte de fond, et les segments — déjà
/// mis en forme — qu'il porte, joints par « · » à l'écriture.
///
/// Un compartiment **est** un groupe de la ligne d'avant : le régime, ce qui
/// tourne, où l'on est, ce que la session consomme. Un compartiment sans
/// segment se retire, comme un segment absent se retirait du groupe.
///
/// Il porte ses segments et non un texte joint depuis le lot 2 du chantier —
/// 19/09/2026 au soir — : le repli en largeur coupe entre compartiments, et,
/// quand un compartiment seul déborde encore, entre ses segments. Voir
/// [`empaqueter`].
pub(crate) struct Compartiment {
    /// Composantes `r;g;b` de la teinte, sans le préfixe SGR : la même sert de
    /// fond au compartiment et d'encre aux caps qui le bordent.
    pub(crate) teinte: &'static str,
    /// Segments non vides, dans l'ordre de la ligne.
    pub(crate) segments: Vec<String>,
}

impl Compartiment {
    /// Construit un compartiment à partir de segments optionnels : les absents
    /// et les vides se retirent, le compartiment aussi s'il n'en reste aucun.
    pub(crate) fn nouveau(teinte: &'static str, segments: Vec<Option<String>>) -> Self {
        Compartiment {
            teinte,
            segments: segments
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }

    fn est_vide(&self) -> bool {
        self.segments.is_empty()
    }

    /// Texte du compartiment : ses segments joints par le séparateur de
    /// segment — c'est la jointure que `joindre_groupes` faisait au rang du
    /// groupe, avant la capsule.
    fn texte(&self) -> String {
        self.segments.join(&separateur())
    }

    /// Largeur du compartiment à l'écran, liserés non compris.
    fn largeur(&self) -> usize {
        largeur_visible(&self.texte())
    }
}

/// Compte les cellules qu'un texte occupe à l'écran : ses caractères, moins les
/// séquences `ESC[…m` qui n'en occupent aucune.
///
/// Tous les glyphes de la ligne font une cellule — chiffres, lettres
/// accentuées, blocs de densité, flèches, caps Powerline compris —, si bien
/// que compter les caractères suffit une fois les séquences retirées. Une
/// séquence CSI court de `ESC[` à son octet final, dans `@`–`~` ; un `ESC`
/// orphelin compte pour rien.
pub(crate) fn largeur_visible(texte: &str) -> usize {
    let mut largeur = 0;
    let mut caracteres = texte.chars().peekable();
    while let Some(c) = caracteres.next() {
        if c != '\u{1b}' {
            largeur += 1;
            continue;
        }
        if caracteres.peek() == Some(&'[') {
            caracteres.next();
            for suite in caracteres.by_ref() {
                if ('@'..='~').contains(&suite) {
                    break;
                }
            }
        }
    }
    largeur
}

/// Largeur d'une capsule à l'écran : les deux caps, chaque compartiment avec
/// ses deux liserés, une jonction entre deux compartiments.
fn largeur_capsule(compartiments: &[&Compartiment]) -> usize {
    if compartiments.is_empty() {
        return 0;
    }
    2 + compartiments.iter().map(|c| c.largeur() + 2).sum::<usize>() + (compartiments.len() - 1)
}

/// Indique si une capsule tient dans la capacité, une fois `en_plus` ajouté à
/// ses compartiments.
fn tient(courante: &[Compartiment], en_plus: &Compartiment, capacite: usize) -> bool {
    let mut tous: Vec<&Compartiment> = courante.iter().collect();
    tous.push(en_plus);
    largeur_capsule(&tous) <= capacite
}

/// Répartit les rangées prévues en capsules qui tiennent dans la capacité,
/// sans rien retirer : ce qui ne tient pas sur un rang passe au rang suivant,
/// à une frontière propre. Deux rangées prévues ne se rejoignent jamais ; une
/// rangée qui tient reste entière — et l'assemblage n'en prévoit qu'une, si
/// bien que la ligne reste **une** tant que la largeur le permet.
///
/// Deux frontières de coupe, dans l'ordre. **Entre compartiments** d'abord : la
/// capsule peut devenir « tête + identité + emplacement » puis « mesures », ou
/// plus finement encore. **Entre segments** ensuite, quand un compartiment seul
/// déborde encore : ses segments remplissent ce qui reste de la capsule en
/// cours, puis continuent dans la suivante, chaque morceau gardant la teinte du
/// compartiment. Un segment seul plus large que la capacité s'émet tel quel,
/// dans sa propre capsule : il s'enroulera, c'est le cas où il n'y a plus rien
/// à faire.
///
/// Sans capacité — pas de console à mesurer —, les rangées sortent telles
/// quelles, vidées de leurs compartiments sans segment.
///
/// Le nombre de rangs croît donc avec l'étroitesse de la fenêtre, comme le
/// ferait l'enroulement que Claude Code applique à une ligne trop large ; mais
/// chaque rang est une capsule entière, caps compris.
fn empaqueter(rangees: Vec<Vec<Compartiment>>, capacite: Option<usize>) -> Vec<Vec<Compartiment>> {
    let Some(capacite) = capacite else {
        return rangees
            .into_iter()
            .map(|rangee| rangee.into_iter().filter(|c| !c.est_vide()).collect())
            .collect();
    };

    let mut capsules: Vec<Vec<Compartiment>> = Vec::new();
    for rangee in rangees {
        let mut courante: Vec<Compartiment> = Vec::new();
        for compartiment in rangee.into_iter().filter(|c| !c.est_vide()) {
            // Le compartiment entier, à la suite.
            if tient(&courante, &compartiment, capacite) {
                courante.push(compartiment);
                continue;
            }
            // Entier encore, mais en tête d'une capsule neuve.
            if !courante.is_empty() && tient(&[], &compartiment, capacite) {
                capsules.push(std::mem::take(&mut courante));
                courante.push(compartiment);
                continue;
            }
            // Segment par segment : on remplit ce qui reste, puis on continue
            // dans la capsule suivante, sous la même teinte.
            let teinte = compartiment.teinte;
            let mut partiel = Compartiment {
                teinte,
                segments: Vec::new(),
            };
            for segment in compartiment.segments {
                let mut essai = Compartiment {
                    teinte,
                    segments: partiel.segments.clone(),
                };
                essai.segments.push(segment.clone());
                if tient(&courante, &essai, capacite) {
                    partiel = essai;
                    continue;
                }
                if !partiel.est_vide() {
                    courante.push(partiel);
                }
                if !courante.is_empty() {
                    capsules.push(std::mem::take(&mut courante));
                }
                // Même trop large pour une capsule vide : il part seul.
                partiel = Compartiment {
                    teinte,
                    segments: vec![segment],
                };
            }
            if !partiel.est_vide() {
                courante.push(partiel);
            }
        }
        if !courante.is_empty() {
            capsules.push(courante);
        }
    }
    capsules
}

/// Enveloppe une ligne de compartiments dans une capsule : cap gauche,
/// compartiments joints par une jonction, cap droit — et, depuis le soir du
/// 19/09/2026, un bord sur le rang du dessus et un sur celui du dessous.
///
/// Séquence émise, pour des compartiments A puis B — **trois rangs** :
///
/// ```text
/// ESC[38;2;<C>m ␠ ▁▁▁…▁ ESC[0m
/// ESC[38;2;<C>m <cap gauche>
/// ESC[48;2;<A>m ␠ <texte A> ␠
/// ESC[48;2;<B>m ESC[38;2;<C>m <jonction> ␠ <texte B> ␠
/// ESC[0m ESC[38;2;<C>m <cap droit> ESC[0m
/// ESC[38;2;<C>m ␠ ▔▔▔…▔ ESC[0m
/// ```
///
/// `C` est l'encre du contour — [`RVB_CONTOUR`]. Les deux caps et les
/// jonctions sont des arcs tracés dans cette encre, sur le fond de ce qui suit
/// — le terminal pour les caps, le compartiment suivant pour la jonction. Les
/// deux bords — [`BORD_HAUT`], [`BORD_BAS`] — occupent les rangs voisins, de
/// la colonne qui suit le cap gauche à celle qui précède le cap droit : ils
/// affleurent les bords de la pilule là où les arcs se terminent, et ferment
/// la forme. Ils s'ouvrent par la séquence d'encre, jamais par le blanc :
/// Claude Code rogne chaque ligne avant de l'afficher, et un blanc en tête
/// décalerait le bord d'une colonne. Les liserés sont des blancs réels, hors
/// de toute séquence, qui héritent du fond ouvert. Sur le rang de la pilule,
/// les deux seules remises à zéro encadrent le cap droit : la première referme
/// le fond pour que le cap se dessine sur le terminal, la seconde referme tout.
///
/// La jonction s'émet même entre deux compartiments de même teinte — Q7 du
/// chantier : une géométrie constante vaut mieux qu'une colonne gagnée dans un
/// cas rare — et, depuis que l'arc a son encre, elle n'y est plus invisible.
///
/// Sous `NO_COLOR`, les textes joints par un espace : la forme se retire, le
/// contenu reste — c'était déjà la frontière tenue par les pastilles.
///
/// La décision de coloration est passée en paramètre : le seul appelant est
/// [`assembler_capsules_selon`], qui la tient déjà et qui, sous `NO_COLOR`,
/// n'appelle même pas cette fonction — la ligne unique s'y compose directement.
fn encapsuler(compartiments: &[Compartiment], sans_couleur: bool) -> String {
    let pleins: Vec<&Compartiment> = compartiments.iter().filter(|c| !c.est_vide()).collect();
    if pleins.is_empty() {
        return String::new();
    }

    if sans_couleur {
        return pleins
            .iter()
            .map(|c| c.texte())
            .collect::<Vec<_>>()
            .join(" ");
    }

    let mut ligne = format!("\u{1b}[38;2;{RVB_CONTOUR}m{CAP_GAUCHE}");
    for (rang, compartiment) in pleins.iter().enumerate() {
        ligne.push_str(&format!("\u{1b}[48;2;{}m", compartiment.teinte));
        if rang > 0 {
            ligne.push_str(&format!("\u{1b}[38;2;{RVB_CONTOUR}m{JONCTION}"));
        }
        ligne.push_str(LISERE);
        ligne.push_str(&compartiment.texte());
        ligne.push_str(LISERE);
    }
    ligne.push_str(&format!(
        "\u{1b}[0m\u{1b}[38;2;{RVB_CONTOUR}m{CAP_DROIT}\u{1b}[0m"
    ));

    // Les bords courent entre les deux caps : la largeur de la capsule moins
    // ses deux extrémités, le cap gauche sauté par un blanc.
    let interieur = largeur_capsule(&pleins) - 2;
    let bord = |glyphe: &str| {
        format!(
            "\u{1b}[38;2;{RVB_CONTOUR}m {}\u{1b}[0m",
            glyphe.repeat(interieur)
        )
    };
    format!("{}\n{ligne}\n{}", bord(BORD_HAUT), bord(BORD_BAS))
}

/// Assemble la sortie : les rangées prévues, repliées à la capacité du
/// terminal par [`empaqueter`], une capsule par rang, jointes par un saut de
/// ligne, les rangs vides retirés.
///
/// `capacite` est la largeur de la console moins la marge — voir
/// [`MARGE_LARGEUR`](crate::reglages::MARGE_LARGEUR) —, ou `None` quand le
/// processus n'a pas de console : les rangées sortent alors telles quelles.
///
/// Sous `NO_COLOR`, **une seule ligne** : tous les compartiments à la suite,
/// joints par un espace, dans l'ordre des rangées, et sans repli — le terminal
/// enroule, comme il enroulait la ligne d'avant. Garder les rangs aurait été
/// plus fidèle à l'affichage coloré, mais aurait coûté l'égalité octet pour
/// octet avec l'oracle sur les cas non colorés du harnais — pour un mode
/// « dégradé, jamais celui qu'on regarde ». Le filet l'emporte.
pub(crate) fn assembler_capsules(
    rangees: Vec<Vec<Compartiment>>,
    capacite: Option<usize>,
) -> String {
    assembler_capsules_selon(rangees, capacite, sans_couleur())
}

/// Cœur d'[`assembler_capsules`], la décision de coloration passée en
/// paramètre.
fn assembler_capsules_selon(
    rangees: Vec<Vec<Compartiment>>,
    capacite: Option<usize>,
    sans_couleur: bool,
) -> String {
    if sans_couleur {
        return rangees
            .iter()
            .flatten()
            .filter(|c| !c.est_vide())
            .map(|c| c.texte())
            .collect::<Vec<_>>()
            .join(" ");
    }
    empaqueter(rangees, capacite)
        .iter()
        .map(|rang| encapsuler(rang, false))
        .filter(|capsule| !capsule.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reglages::{RVB_LIEU, RVB_TETE};

    const ESC: &str = "\u{1b}";

    /// Un compartiment d'un seul segment, tel que le voit la capsule.
    fn compartiment(teinte: &'static str, texte: &str) -> Compartiment {
        Compartiment::nouveau(teinte, vec![Some(texte.to_string())])
    }

    #[test]
    fn le_palier_se_lit_du_plus_haut_au_plus_bas() {
        assert_eq!(palier(79, 80, 90), Palier::Aucun);
        assert_eq!(palier(80, 80, 90), Palier::Alerte);
        assert_eq!(palier(90, 80, 90), Palier::Critique);
        // Seuil critique mal réglé, sous le seuil d'alerte : le rouge l'emporte
        // plutôt que de rendre un palier inatteignable.
        assert_eq!(palier(85, 90, 80), Palier::Critique);
        // Le pire de deux paliers est leur maximum.
        assert_eq!(Palier::Aucun.max(Palier::Alerte), Palier::Alerte);
        assert_eq!(Palier::Critique.max(Palier::Alerte), Palier::Critique);
        assert_eq!(teinte_du_palier(Palier::Aucun), RVB_CORPS);
        assert_eq!(teinte_du_palier(Palier::Alerte), RVB_ALERTE);
        assert_eq!(teinte_du_palier(Palier::Critique), RVB_CRITIQUE);
    }

    #[test]
    fn jauge_monte_par_septiemes() {
        // Un cran tous les 14,3 points, les cent points étant découpés sur la
        // longueur de la table.
        assert_eq!(bloc_jauge(0), "░░");
        assert_eq!(bloc_jauge(14), "░░");
        assert_eq!(bloc_jauge(15), "▒░");
        assert_eq!(bloc_jauge(28), "▒░");
        assert_eq!(bloc_jauge(29), "▓░");
        assert_eq!(bloc_jauge(43), "█░");
        assert_eq!(bloc_jauge(58), "█▒");
        assert_eq!(bloc_jauge(72), "█▓");
        assert_eq!(bloc_jauge(86), "██");
        assert_eq!(bloc_jauge(100), "██");
        // Hors bornes : les extrémités, jamais d'indexation hors table.
        assert_eq!(bloc_jauge(-30), "░░");
        assert_eq!(bloc_jauge(150), "██");
    }

    /// La cellule de gauche se remplit avant celle de droite, et la jauge ne
    /// redescend jamais.
    ///
    /// C'est ce qui autorise à la lire comme un remplissage plutôt qu'à
    /// comparer deux densités entre elles — et ce qui limite l'échelle à sept
    /// crans, un huitième ne pouvant s'y insérer sans arbitraire.
    #[test]
    fn les_crans_de_jauge_sont_strictement_croissants() {
        /// Rang d'une densité, du vide au plein.
        fn rang(cellule: char) -> usize {
            match cellule {
                '░' => 0,
                '▒' => 1,
                '▓' => 2,
                '█' => 3,
                autre => panic!("densité inattendue : {:?}", autre),
            }
        }

        let mut precedent: Option<(usize, usize)> = None;
        for cran in BLOCS_JAUGE {
            let mut cellules = cran.chars();
            let gauche = rang(cellules.next().expect("cellule de gauche"));
            let droite = rang(cellules.next().expect("cellule de droite"));
            assert_eq!(cellules.next(), None, "un cran fait deux cellules");

            // La droite ne se remplit pas avant que la gauche ne soit pleine.
            assert!(
                droite == 0 || gauche == 3,
                "le cran {:?} remplit la droite avant la gauche",
                cran
            );

            if let Some(avant) = precedent {
                assert!(
                    (gauche, droite) > avant,
                    "le cran {:?} ne succède pas à {:?}",
                    cran,
                    avant
                );
            }
            precedent = Some((gauche, droite));
        }
    }

    /// Aucun cran de la jauge ne doit sortir de la police du terminal.
    ///
    /// C'est la vérification que le commentaire d'origine affirmait sans
    /// l'avoir faite : les six blocs de hauteur retirés le 25/08/2026 étaient
    /// absents de Consolas, et le terminal les faisait dessiner par une police
    /// de repli, à une autre graisse. Les blocs partiels `▏▎▍▌▋▊▉`, qui
    /// donneraient seize crans, en sont absents tout autant.
    #[test]
    fn les_crans_de_jauge_sont_dans_consolas() {
        // Sous-ensemble de « Block Elements » que Consolas possède réellement,
        // relevé dans sa table de glyphes — élargi le 19/09/2026 aux demi-blocs
        // et au bloc supérieur, relevés à leur tour ; la jauge n'en use pas.
        const DANS_CONSOLAS: [char; 8] = ['░', '▒', '▓', '█', '▄', '▀', '▌', '▐'];

        for cran in BLOCS_JAUGE {
            for cellule in cran.chars() {
                assert!(
                    DANS_CONSOLAS.contains(&cellule),
                    "le cran {:?} porte {:?}, qui n'est pas dans Consolas : il \
                     serait dessiné par une police de repli, à une autre graisse \
                     et un autre alignement",
                    cran,
                    cellule
                );
            }
        }
    }

    /// Le vide de la jauge est en piste, le plein dans l'encre du palier, et
    /// chaque cellule porte sa propre séquence.
    #[test]
    fn la_jauge_se_colore_cellule_par_cellule() {
        assert_eq!(
            colorer_jauge_selon("▒░", Palier::Aucun, false),
            format!("{ESC}[{CODE_TEXTE}m▒{ESC}[{CODE_PISTE}m░")
        );
        assert_eq!(
            colorer_jauge_selon("█▓", Palier::Alerte, false),
            format!("{ESC}[{CODE_ALERTE}m█{ESC}[{CODE_ALERTE}m▓")
        );
        // Au palier critique, l'encre corail sans gras : l'épaisseur est un
        // signal pour un nombre, pas pour un aplat.
        let critique = colorer_jauge_selon("██", Palier::Critique, false);
        assert_eq!(
            critique,
            format!("{ESC}[{CODE_CRITIQUE}m█{ESC}[{CODE_CRITIQUE}m█")
        );
        assert!(!critique.contains("[1m"));
        // Sous NO_COLOR, le cran nu.
        assert_eq!(colorer_jauge_selon("▒░", Palier::Aucun, true), "▒░");
    }

    #[test]
    fn mesure_jaugee_place_le_bloc_entre_libelle_et_valeur() {
        // Les attendus passent par les mêmes briques que la mise en forme :
        // les deux côtés suivent ainsi la même décision `NO_COLOR`, que Claude
        // Code définit dans l'environnement de `cargo test`.
        let (jaugee, palier_jaugee) = formater_mesure_jaugee("5h", "20", "%", 20, 80, 90);
        assert_eq!(
            jaugee,
            format!(
                "{} {} {}{}",
                attenuer("5h"),
                colorer_jauge("▒░", Palier::Aucun),
                texte_plein("20"),
                attenuer("%")
            )
        );
        assert_eq!(palier_jaugee, Palier::Aucun);

        // Le contexte, lui, garde la forme d'origine.
        assert_eq!(
            formater_mesure("ctx", "22", "%", 22, 80, 90).0,
            format!("{} {}{}", attenuer("ctx"), texte_plein("22"), attenuer("%"))
        );
        // Un chiffre de moins : le calage rend la colonne perdue, pour que ce
        // qui suit ne glisse pas d'un rafraîchissement à l'autre.
        assert_eq!(
            formater_mesure("ctx", "9", "%", 9, 80, 90).0,
            format!("{}  {}{}", attenuer("ctx"), texte_plein("9"), attenuer("%"))
        );
        // Le champ est étalonné sur « 99% » : le plafond et la valeur mémorisée
        // débordent d'un caractère, ce qui est le prix assumé du calage court.
        assert_eq!(
            formater_mesure("ctx", "100", "%", 100, 80, 90).0,
            format!(
                "{} {}{}",
                attenuer("ctx"),
                au_palier("100", Palier::Critique),
                attenuer("%")
            )
        );
    }

    /// L'unité est du chrome, la valeur ne l'est pas : la valeur porte le
    /// palier critique, l'unité le gris, et la mesure remonte son palier.
    #[test]
    fn l_unite_reste_hors_de_la_couleur_du_palier() {
        let (mesure, palier) = formater_mesure_ornee("ctx", false, "93", "%", 93, 80, 90);

        assert!(mesure.contains(&au_palier("93", Palier::Critique)));
        assert!(mesure.ends_with(&attenuer("%")));
        assert_eq!(palier, Palier::Critique);
    }

    /// Le gras referme ce qu'il ouvre, et rien d'autre : jamais de `ESC[0m`
    /// dans un fragment.
    #[test]
    fn le_gras_referme_ce_qu_il_ouvre() {
        assert_eq!(gras_selon("94", false), format!("{ESC}[1m94{ESC}[22m"));
        assert_eq!(gras_selon("94", true), "94");

        let critique = au_palier_selon("94", Palier::Critique, false);
        assert_eq!(
            critique,
            format!("{ESC}[1m{ESC}[{CODE_CRITIQUE}m94{ESC}[22m")
        );
        assert!(!critique.contains("[0m"));

        // Les deux autres paliers n'ouvrent qu'une couleur, et ne referment
        // rien : c'est la capsule qui referme.
        assert_eq!(
            au_palier_selon("83", Palier::Alerte, false),
            format!("{ESC}[{CODE_ALERTE}m83")
        );
        assert_eq!(
            au_palier_selon("34", Palier::Aucun, false),
            format!("{ESC}[{CODE_TEXTE}m34")
        );
        assert_eq!(
            colorer_selon("x", "38;2;1;2;3", false),
            format!("{ESC}[38;2;1;2;3mx")
        );
        assert_eq!(colorer_selon("x", "38;2;1;2;3", true), "x");
    }

    /// L'invariant central de la capsule : trois rangs, dont celui de la pilule
    /// porte exactement deux remises à zéro, toutes deux autour du cap droit,
    /// et chaque compartiment ouvert par son fond puis un liseré.
    #[test]
    fn la_capsule_ne_remet_a_zero_qu_apres_le_cap_droit() {
        let capsule = encapsuler(
            &[
                compartiment(RVB_TETE, "Pro"),
                compartiment(RVB_CORPS, "v2.1.259 · Opus 5"),
                compartiment(RVB_LIEU, "PY_xl"),
            ],
            false,
        );
        let rangs: Vec<&str> = capsule.split('\n').collect();
        assert_eq!(rangs.len(), 3, "{capsule:?}");
        let pilule = rangs[1];

        let remise = format!("{ESC}[0m");
        assert_eq!(pilule.matches(&remise).count(), 2);
        let fin = format!("{ESC}[0m{ESC}[38;2;{RVB_CONTOUR}m{CAP_DROIT}{ESC}[0m");
        assert!(pilule.ends_with(&fin), "{pilule:?}");
        assert_eq!(pilule.find(&remise), Some(pilule.len() - fin.len()));

        // Le cap gauche dans l'encre du contour, sur le fond du terminal —
        // aucun fond posé avant lui —, puis le premier fond et son liseré.
        assert!(pilule.starts_with(&format!(
            "{ESC}[38;2;{RVB_CONTOUR}m{CAP_GAUCHE}{ESC}[48;2;{RVB_TETE}m{LISERE}Pro{LISERE}"
        )));
        // La jonction dans l'encre du contour, sur le fond du suivant, puis le
        // liseré.
        assert!(pilule.contains(&format!(
            "{ESC}[48;2;{RVB_CORPS}m{ESC}[38;2;{RVB_CONTOUR}m{JONCTION}{LISERE}v2.1.259"
        )));
        assert!(pilule.contains(&format!(
            "{ESC}[48;2;{RVB_LIEU}m{ESC}[38;2;{RVB_CONTOUR}m{JONCTION}{LISERE}PY_xl{LISERE}"
        )));
        // Trois fonds posés, un par compartiment ; aucun attribut de trait.
        assert_eq!(pilule.matches(&format!("{ESC}[48;2;")).count(), 3);
        for attribut in ["[4m", "[53m", "[58;2;"] {
            assert!(!capsule.contains(&format!("{ESC}{attribut}")), "{attribut}");
        }
    }

    /// Les deux bords du cadre : un rang au-dessus, un au-dessous, dans l'encre
    /// du contour, ouverts par la séquence et non par le blanc, larges comme la
    /// pilule moins ses deux caps.
    #[test]
    fn les_bords_du_cadre_encadrent_la_pilule_entre_les_caps() {
        let capsule = encapsuler(
            &[
                compartiment(RVB_TETE, "Pro"),
                compartiment(RVB_CORPS, "v2.1.259 · Opus 5"),
            ],
            false,
        );
        let rangs: Vec<&str> = capsule.split('\n').collect();
        assert_eq!(rangs.len(), 3, "{capsule:?}");
        let (haut, pilule, bas) = (rangs[0], rangs[1], rangs[2]);

        // La pilule : deux caps, deux compartiments avec leurs liserés, une
        // jonction.
        let interieur = (3 + 2) + (17 + 2) + 1;
        assert_eq!(largeur_visible(pilule), interieur + 2, "{pilule:?}");
        // Les bords : un blanc pour sauter le cap gauche, puis le glyphe sur
        // toute la largeur intérieure ; rien au-dessus du cap droit.
        assert_eq!(
            haut,
            format!(
                "{ESC}[38;2;{RVB_CONTOUR}m {}{ESC}[0m",
                BORD_HAUT.repeat(interieur)
            )
        );
        assert_eq!(
            bas,
            format!(
                "{ESC}[38;2;{RVB_CONTOUR}m {}{ESC}[0m",
                BORD_BAS.repeat(interieur)
            )
        );
        assert_eq!(largeur_visible(haut), interieur + 1);
        // Claude Code rogne chaque ligne : le premier caractère d'un bord est
        // la séquence, jamais le blanc.
        assert!(haut.starts_with(ESC) && bas.starts_with(ESC));
    }

    /// Sous `NO_COLOR`, la capsule est la ligne d'hier : les groupes joints par
    /// un espace, rien d'autre.
    #[test]
    fn sous_no_color_la_capsule_est_la_ligne_d_hier() {
        let groupes = [
            compartiment(RVB_TETE, "Pro"),
            compartiment(RVB_CORPS, "v2.1.259 · Opus 5"),
            compartiment(RVB_LIEU, "PY_xl"),
            compartiment(RVB_CORPS, "ctx 22% · 5h ▒░ 20%"),
        ];
        assert_eq!(
            encapsuler(&groupes, true),
            "Pro v2.1.259 · Opus 5 PY_xl ctx 22% · 5h ▒░ 20%"
        );
        // Deux lignes en couleur, une seule sans : la forme se retire, le
        // contenu reste dans l'ordre.
        let haute = vec![
            compartiment(RVB_TETE, "Pro"),
            compartiment(RVB_CORPS, "v2.1.259 · Opus 5"),
            compartiment(RVB_LIEU, "PY_xl"),
        ];
        let basse = vec![compartiment(RVB_CORPS, "ctx 22% · 5h ▒░ 20%")];
        assert_eq!(
            assembler_capsules_selon(vec![haute, basse], None, true),
            "Pro v2.1.259 · Opus 5 PY_xl ctx 22% · 5h ▒░ 20%"
        );
    }

    /// Un compartiment vide se retire avec sa jonction ; une capsule sans
    /// compartiment ne s'émet pas.
    #[test]
    fn un_compartiment_vide_se_retire_avec_sa_jonction() {
        let capsule = encapsuler(
            &[
                compartiment(RVB_TETE, ""),
                compartiment(RVB_CORPS, "Opus 5"),
                compartiment(RVB_LIEU, ""),
            ],
            false,
        );
        // Sans abonnement, la capsule s'ouvre sur le corps (Q4) ; une seule
        // jonction possible, aucune émise. Un seul compartiment : les bords
        // font sa largeur plus ses deux liserés.
        let bord =
            |glyphe: &str| format!("{ESC}[38;2;{RVB_CONTOUR}m {}{ESC}[0m", glyphe.repeat(6 + 2));
        assert_eq!(
            capsule,
            format!(
                "{}\n{ESC}[38;2;{RVB_CONTOUR}m{CAP_GAUCHE}{ESC}[48;2;{RVB_CORPS}m{LISERE}Opus 5{LISERE}{ESC}[0m{ESC}[38;2;{RVB_CONTOUR}m{CAP_DROIT}{ESC}[0m\n{}",
                bord(BORD_HAUT),
                bord(BORD_BAS)
            )
        );
        // La jonction et le cap droit sont le même glyphe : une seule
        // occurrence, celle du cap.
        assert_eq!(capsule.matches(JONCTION).count(), 1);

        assert_eq!(encapsuler(&[compartiment(RVB_TETE, "")], false), "");
        assert_eq!(encapsuler(&[], false), "");
        assert_eq!(encapsuler(&[compartiment(RVB_TETE, "")], true), "");
    }

    /// Entre deux compartiments de même teinte, la jonction s'émet quand même
    /// (Q7) : la géométrie ne dépend pas des teintes — et l'arc, qui a son
    /// encre, y reste visible.
    #[test]
    fn la_jonction_s_emet_meme_entre_deux_fonds_egaux() {
        let capsule = encapsuler(
            &[
                compartiment(RVB_CORPS, "Opus 5"),
                compartiment(RVB_CORPS, "ctx 22%"),
            ],
            false,
        );
        assert!(capsule.contains(&format!(
            "{ESC}[48;2;{RVB_CORPS}m{ESC}[38;2;{RVB_CONTOUR}m{JONCTION}"
        )));
    }

    /// Deux capsules, trois rangs chacune, jointes par un seul saut de ligne ;
    /// chaque pilule referme la sienne.
    #[test]
    fn deux_capsules_une_par_ligne() {
        let sortie = assembler_capsules_selon(
            vec![
                vec![
                    compartiment(RVB_TETE, "Pro"),
                    compartiment(RVB_CORPS, "Opus 5"),
                ],
                vec![compartiment(RVB_ALERTE, "ctx 22%")],
            ],
            None,
            false,
        );
        let lignes: Vec<&str> = sortie.split('\n').collect();
        assert_eq!(lignes.len(), 6, "{sortie:?}");
        for pilule in [lignes[1], lignes[4]] {
            assert_eq!(pilule.matches(&format!("{ESC}[0m")).count(), 2);
            assert!(pilule.ends_with(&format!("{CAP_DROIT}{ESC}[0m")));
        }
        for bord in [lignes[0], lignes[2], lignes[3], lignes[5]] {
            assert!(
                bord.starts_with(&format!("{ESC}[38;2;{RVB_CONTOUR}m ")),
                "{bord:?}"
            );
        }
        assert!(lignes[0].contains(BORD_HAUT) && lignes[3].contains(BORD_HAUT));
        assert!(lignes[2].contains(BORD_BAS) && lignes[5].contains(BORD_BAS));
        // La capsule basse porte la teinte du palier sur son fond ; ses caps,
        // eux, restent dans l'encre du contour comme ceux de la haute.
        assert!(lignes[4].starts_with(&format!("{ESC}[38;2;{RVB_CONTOUR}m{CAP_GAUCHE}")));
        assert!(lignes[4].contains(&format!("{ESC}[48;2;{RVB_ALERTE}m{LISERE}ctx 22%")));
        assert!(lignes[4].ends_with(&format!(
            "{ESC}[0m{ESC}[38;2;{RVB_CONTOUR}m{CAP_DROIT}{ESC}[0m"
        )));
    }

    /// Une ligne dont tous les compartiments se retirent ne s'émet pas : ni
    /// saut de ligne surnuméraire, ni capsule vide.
    #[test]
    fn la_ligne_basse_ne_s_emet_pas_vide() {
        let sortie = assembler_capsules_selon(
            vec![
                vec![compartiment(RVB_CORPS, "Opus 5")],
                vec![compartiment(RVB_CORPS, "")],
            ],
            None,
            false,
        );
        let rangs: Vec<&str> = sortie.split('\n').collect();
        assert_eq!(rangs.len(), 3, "{sortie:?}");
        assert!(rangs[1].ends_with(&format!("{CAP_DROIT}{ESC}[0m")));

        // Tout vide : la chaîne l'est aussi, et c'est confirmer_ligne qui
        // rattrape.
        assert_eq!(
            assembler_capsules_selon(vec![vec![compartiment(RVB_CORPS, "")], vec![]], None, false),
            ""
        );
        assert_eq!(assembler_capsules_selon(vec![], None, true), "");
    }

    // -----------------------------------------------------------------------
    // Repli en largeur — lot 2, 19/09/2026
    // -----------------------------------------------------------------------

    /// Un compartiment de plusieurs segments, tel que l'assemblage le construit.
    fn compartiment_segments(teinte: &'static str, segments: &[&str]) -> Compartiment {
        Compartiment::nouveau(
            teinte,
            segments.iter().map(|s| Some((*s).to_string())).collect(),
        )
    }

    /// Textes des compartiments d'un rang, pour lire un empaquetage d'un coup
    /// d'œil.
    fn textes(rangs: &[Vec<Compartiment>]) -> Vec<Vec<String>> {
        rangs
            .iter()
            .map(|rang| rang.iter().map(|c| c.texte()).collect())
            .collect()
    }

    /// La largeur visible ignore les séquences, et compte un par glyphe — blocs,
    /// flèches, caps et lettres accentuées compris.
    #[test]
    fn la_largeur_visible_ignore_les_sequences() {
        assert_eq!(largeur_visible("ctx 34%"), 7);
        assert_eq!(
            largeur_visible(&format!(
                "{ESC}[38;2;1;2;3mctx{ESC}[0m {ESC}[1m34{ESC}[22m%"
            )),
            7
        );
        assert_eq!(largeur_visible("5h ▒░ 20% → 41% épuisé"), 22);
        assert_eq!(largeur_visible(&format!("{CAP_GAUCHE}{CAP_DROIT}")), 2);
        // Un ESC orphelin, ou suivi d'autre chose qu'un crochet, ne compte pas
        // — et n'avale rien.
        assert_eq!(largeur_visible(&format!("a{ESC}b")), 2);
        assert_eq!(largeur_visible(""), 0);

        // La capsule entière : deux caps, deux liserés par compartiment, une
        // jonction — c'est la formule de `largeur_capsule`.
        let rang = [
            compartiment(RVB_TETE, "Pro"),
            compartiment(RVB_CORPS, "Opus 5"),
        ];
        let capsule = encapsuler(&rang, false);
        let pilule = capsule.split('\n').nth(1).expect("rang de la pilule");
        assert_eq!(largeur_visible(pilule), 2 + (3 + 2) + 1 + (6 + 2));
        assert_eq!(
            largeur_visible(pilule),
            largeur_capsule(&rang.iter().collect::<Vec<_>>())
        );
    }

    /// Le compartiment joint ses segments par le séparateur, retire les absents
    /// et les vides, et se retire lui-même s'il n'en reste aucun.
    #[test]
    fn le_compartiment_joint_ses_segments_par_le_point() {
        let c = Compartiment::nouveau(
            RVB_CORPS,
            vec![
                Some("v2.1.259".into()),
                None,
                Some(String::new()),
                Some("Opus 5".into()),
            ],
        );
        assert_eq!(c.texte(), format!("v2.1.259{}Opus 5", separateur()));
        assert!(!c.est_vide());
        assert!(Compartiment::nouveau(RVB_CORPS, vec![None, Some(String::new())]).est_vide());
        assert!(Compartiment::nouveau(RVB_CORPS, vec![]).est_vide());
    }

    /// Sans capacité, les rangées sortent telles quelles, vidées de leurs
    /// compartiments sans segment ; avec une capacité suffisante, de même.
    #[test]
    fn une_rangee_qui_tient_reste_entiere() {
        let rangees = || {
            vec![
                vec![
                    compartiment(RVB_TETE, "Pro"),
                    compartiment(RVB_CORPS, ""),
                    compartiment_segments(RVB_CORPS, &["v2.1.259", "Opus 5 xhigh"]),
                    compartiment(RVB_LIEU, "PY_xl"),
                ],
                vec![compartiment_segments(
                    RVB_CORPS,
                    &["ctx 34%", "5h ▒░ 20% 15:00"],
                )],
            ]
        };
        let attendu = vec![
            vec![
                "Pro".to_string(),
                format!("v2.1.259{}Opus 5 xhigh", separateur()),
                "PY_xl".to_string(),
            ],
            vec![format!("ctx 34%{}5h ▒░ 20% 15:00", separateur())],
        ];
        assert_eq!(textes(&empaqueter(rangees(), None)), attendu);
        // Rang haut : 2 + (3+2) + 1 + (23+2) + 1 + (5+2) = 41 ; il tient à 41.
        assert_eq!(textes(&empaqueter(rangees(), Some(41))), attendu);
        assert_eq!(textes(&empaqueter(rangees(), Some(120))), attendu);
    }

    /// Trop large d'une cellule : la rangée se coupe entre compartiments, et
    /// jamais à l'intérieur tant qu'un compartiment entier tient seul.
    #[test]
    fn une_rangee_trop_large_se_coupe_entre_compartiments() {
        let rangees = || {
            vec![vec![
                compartiment(RVB_TETE, "Pro"),
                compartiment_segments(RVB_CORPS, &["v2.1.259", "Opus 5 xhigh"]),
                compartiment(RVB_LIEU, "PY_xl"),
            ]]
        };
        // 40, une cellule de moins que les 41 nécessaires : l'emplacement passe
        // au rang suivant, entier.
        assert_eq!(
            textes(&empaqueter(rangees(), Some(40))),
            vec![
                vec![
                    "Pro".to_string(),
                    format!("v2.1.259{}Opus 5 xhigh", separateur())
                ],
                vec!["PY_xl".to_string()],
            ]
        );
        // 31 : la tête tient avec le début de l'identité ? Non — l'identité
        // entière tient seule dans une capsule neuve (2 + 25 = 27), elle y va
        // entière plutôt que coupée.
        assert_eq!(
            textes(&empaqueter(rangees(), Some(31))),
            vec![
                vec!["Pro".to_string()],
                vec![format!("v2.1.259{}Opus 5 xhigh", separateur())],
                vec!["PY_xl".to_string()],
            ]
        );
    }

    /// Un compartiment plus large que la capacité se coupe entre ses segments :
    /// les premiers remplissent ce qui reste du rang en cours, les suivants
    /// continuent au rang suivant, sous la même teinte.
    #[test]
    fn un_compartiment_trop_large_se_coupe_entre_segments() {
        let mesures = || {
            vec![vec![compartiment_segments(
                RVB_ALERTE,
                &["ctx 34%", "5h ▒░ 20% → 41% 15:00", "7j █░ 45%"],
            )]]
        };
        // Largeurs : ctx 7, 5h 21, 7j 9 ; séparateur 3 ; capsule à un
        // compartiment : 2 + largeur + 2.
        // À 40 : « ctx · 5h » fait 7+3+21 = 31, capsule 35 ; avec 7j, 43 > 40.
        let rangs = empaqueter(mesures(), Some(40));
        assert_eq!(
            textes(&rangs),
            vec![
                vec![format!("ctx 34%{}5h ▒░ 20% → 41% 15:00", separateur())],
                vec!["7j █░ 45%".to_string()],
            ]
        );
        assert!(rangs.iter().flatten().all(|c| c.teinte == RVB_ALERTE));
        // À 20 : chaque segment seul ; le second, à 21 + 4 = 25, déborde et
        // part quand même seul — il s'enroulera, il n'y a plus rien à faire.
        assert_eq!(
            textes(&empaqueter(mesures(), Some(20))),
            vec![
                vec!["ctx 34%".to_string()],
                vec!["5h ▒░ 20% → 41% 15:00".to_string()],
                vec!["7j █░ 45%".to_string()],
            ]
        );
    }

    /// Les premiers segments d'un compartiment trop large remplissent le rang
    /// en cours avant d'en ouvrir un autre.
    #[test]
    fn les_segments_remplissent_le_rang_en_cours_avant_d_en_ouvrir_un() {
        let rangee = vec![vec![
            compartiment(RVB_TETE, "Pro"),
            compartiment_segments(RVB_CORPS, &["v2.1.259", "Opus 5 xhigh", "fast"]),
        ]];
        // « Pro » + « v2.1.259 » : 2 + 5 + 1 + 10 = 18 ; avec « · Opus 5 xhigh »,
        // 33. À 23, la version reste avec la tête ; « Opus 5 xhigh · fast »
        // fait 19, soit 23 en capsule : il tient juste au rang suivant.
        let rangs = empaqueter(rangee, Some(23));
        assert_eq!(
            textes(&rangs),
            vec![
                vec!["Pro".to_string(), "v2.1.259".to_string()],
                vec![format!("Opus 5 xhigh{}fast", separateur())],
            ]
        );
        // Les morceaux gardent la teinte du compartiment d'origine.
        assert_eq!(rangs[0][1].teinte, RVB_CORPS);
        assert_eq!(rangs[1][0].teinte, RVB_CORPS);
    }

    /// Les rangées prévues ne se mélangent jamais : les mesures ne remontent
    /// pas sur le rang haut, même s'il y reste de la place.
    #[test]
    fn les_rangees_prevues_ne_se_melangent_pas() {
        let rangs = empaqueter(
            vec![
                vec![compartiment(RVB_TETE, "Pro")],
                vec![compartiment(RVB_CORPS, "ctx 34%")],
            ],
            Some(120),
        );
        assert_eq!(
            textes(&rangs),
            vec![vec!["Pro".to_string()], vec!["ctx 34%".to_string()]]
        );
    }

    /// Sous `NO_COLOR`, le repli n'a pas lieu : une seule ligne, quelle que
    /// soit la capacité.
    #[test]
    fn sous_no_color_le_repli_n_a_pas_lieu() {
        let sortie = assembler_capsules_selon(
            vec![
                vec![
                    compartiment(RVB_TETE, "Pro"),
                    compartiment_segments(RVB_CORPS, &["v2.1.259", "Opus 5"]),
                ],
                vec![compartiment(RVB_CORPS, "ctx 34%")],
            ],
            Some(10),
            true,
        );
        assert_eq!(
            sortie,
            format!("Pro v2.1.259{}Opus 5 ctx 34%", separateur())
        );
    }
}
