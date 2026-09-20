//! Ligne de statut Claude Code : abonnement, modèle, effort, emplacement,
//! contexte et fenêtres de limitation.
//!
//! Claude Code transmet sur l'entrée standard un objet JSON décrivant la
//! session en cours, et affiche telle quelle la ligne écrite sur la sortie
//! standard.
//!
//! Les champs effectivement lus, relevés sur Claude Code 2.1.245 :
//!   - `model.display_name`                    : nom lisible, ex. « Opus 5 »
//!   - `model.id`                              : identifiant, ex. « claude-fable-5-1 » —
//!                                               en donne la famille, voir [`modele`]
//!   - `effort.level`                          : low|medium|high|xhigh|max
//!   - `fast_mode`                             : mode rapide actif
//!   - `thinking.enabled`                      : réflexion étendue active
//!   - `workspace.current_dir`                 : répertoire de travail
//!   - `workspace.project_dir`                 : répertoire de lancement
//!   - `context_window.used_percentage`        : contexte occupé, en pourcentage
//!   - `context_window.context_window_size`    : fenêtre effective, en jetons
//!   - `rate_limits.<fenêtre>.used_percentage` : consommation, en pourcentage
//!   - `rate_limits.<fenêtre>.resets_at`       : remise à zéro, en secondes Unix
//!   - `version`                               : version chargée par la session
//!
//! Le payload en porte deux fois plus, dont `cost.total_lines_added`,
//! `exceeds_200k_tokens`, `agent.name`, `pr`, `workspace.added_dirs` et
//! `output_style.name` — aucun n'est lu aujourd'hui. Ne pas se fier à cette
//! liste pour ce qu'elle ne dit pas : elle décrit une version, et le contrat se
//! relève sur le poste plutôt qu'il ne se suppose, voir [`capture`].
//!
//! Ce qu'il ne porte **pas**, en revanche, est acquis : la capture du 26/08/2026
//! a relevé trente-quatre champs sur la version 2.1.246, et **aucun** ne dit sur
//! quel abonnement la session tourne. C'est ce relevé qui envoie [`abonnement`]
//! lire le disque plutôt qu'attendre un champ.
//!
//! Les fenêtres de limitation sont au nombre de deux, « five_hour » et
//! « seven_day ». Chacune peut être absente indépendamment de l'autre, et
//! aucune n'est renseignée avant la première réponse de l'API.
//!
//! # Portage
//!
//! Ce binaire remplace `statusline.ps1`, dont il a reproduit le comportement sur
//! les 116 cas du harnais `test-statusline.ps1` — ligne écrite, code de sortie
//! et état du cache compris.
//!
//! Les évolutions se font depuis **côté Rust seul**, le script restant l'oracle
//! et le repli. Jusqu'au 19/09/2026 l'égalité tenait tant qu'aucun cas du
//! harnais n'exerçait le comportement nouveau. **Depuis la capsule, elle ne
//! vaut plus que sous `NO_COLOR`** : la sortie colorée a changé de forme — deux
//! lignes, des fonds, des caps — et l'oracle, gelé (Q2 du chantier), ne la
//! décrit plus ; le harnais le compare avec `-IgnorerCouleur`, et les cas
//! colorés sont couverts par les tests du crate, attendus figés. Sous
//! `NO_COLOR`, la sortie est octet pour octet celle d'avant la capsule, et les
//! 115 cas non colorés le vérifient encore.
//!
//! La raison du portage n'était pas la fonctionnalité mais le coût : 626 ms par
//! rafraîchissement en PowerShell — dont 288 ms de seul démarrage de `pwsh` —
//! contre une quinzaine de millisecondes ici. C'est ce qui rend tenable un
//! `refreshInterval` de quelques secondes, donc une ligne qui ne reste jamais
//! effacée longtemps.
//!
//! # La capsule — 19/09/2026
//!
//! La sortie est **une capsule** à quatre compartiments — le régime, ce qui
//! tourne, d'où l'on parle, ce que la session consomme sur le fond du pire
//! palier atteint —, sur **une ligne tant qu'elle tient** dans la largeur de la
//! console, et dépliée sur autant de rangs qu'il faut sinon, chacun une capsule
//! entière. Depuis le soir du même jour, chaque capsule est **cadrée** : un
//! bord au-dessus, un au-dessous, dans le gris des arcs, soit trois lignes
//! d'écran par capsule. Claude Code affiche toutes les lignes non vides que la
//! commande écrit — chacune rognée de ses blancs —, et replie lui-même celles
//! qui débordent : c'est pour ne pas lui laisser casser la forme que la ligne
//! mesure la console — voir [`largeur`] — et se replie à une frontière propre.
//! Le modèle de rendu — fond par compartiment, fragments qui ne referment que
//! ce qu'ils ouvrent, deux `ESC[0m` par pilule — est décrit en tête de
//! [`sortie`] ; la forme et la palette, dans [`reglages`] ; l'histoire, dans
//! `statusline-rust.md` §23.
//!
//! # Organisation
//!
//! Le portage a d'abord repris les 8 sections du script d'origine dans un
//! fichier unique. Elles sont devenues des modules le 21/08/2026, sans qu'une
//! ligne de logique change — le fichier avoisinait les 1 900 lignes. La
//! correspondance avec l'oracle reste donc lisible section par section :
//!
//! | Module | Section d'origine | Rôle |
//! |---|---|---|
//! | [`reglages`]    | 1     | seuils, couleurs, libellés, noms de fichiers |
//! | [`sortie`]      | 2     | écriture de la ligne, coloration, mise en forme d'une mesure |
//! | [`journal`]     | 2 bis | journal de diagnostic, activé par fichier témoin |
//! | [`capture`]     | 2 ter | relevé du payload reçu, armé par fichier témoin |
//! | [`conversions`] | 3     | lecture défensive des valeurs du payload |
//! | [`temps`]       | 3     | calendrier, heure locale, mise en forme d'un instant |
//! | [`disque`]      | 4     | racine des fichiers d'état, lecture d'un texte |
//! | [`cache`]       | 4     | cache des fenêtres, sérialisé octet pour octet |
//! | [`depot`]       | 5     | branche Git, lue dans « .git/HEAD » |
//! | [`binaire`]     | 5     | version du binaire installé, lue dans ses métadonnées |
//! | [`abonnement`]  | 5     | abonnement du compte, lu dans « ~/.claude.json » |
//! | [`usage`]       | 5     | fenêtre propre au modèle, relevée par `/usage` dans « ~/.claude.json », et sa dérive entre deux relevés |
//! | [`segments`]    | 6     | abonnement, version, modèle, emplacement, contexte |
//! | [`modele`]      | 6     | famille du modèle, facteur de consommation, seuils adaptés |
//! | [`fenetres`]    | 6     | fenêtres de limitation, rythme et projection |
//! | ce fichier      | 7 + 8 | assemblage de la ligne et programme principal |
//!
//! Deux fonctions ont changé de voisinage à cette occasion, parce que leur
//! section d'origine ne les décrivait plus : `formater_mesure` a rejoint
//! [`sortie`], dont elle combine les deux briques, et `formater_instant`
//! [`temps`], dont il met en forme le produit. Les dépendances entre modules
//! restent acycliques, [`reglages`] n'en ayant aucune.
//!
//! Rien n'est jamais fatal. Les valeurs du payload passent par
//! [`conversions::convertir_pourcent`] et [`conversions::convertir_instant`],
//! qui rendent `None` plutôt que d'échouer ; le programme principal, en dernier
//! recours, écrit le nom du modèle seul. Une sortie vide effacerait la ligne
//! dans l'interface, ce qui est pire qu'une information partielle : c'est la
//! raison d'être de ces filets.
//!
//! Ces filets se referment sur un invariant unique, tenu par [`main`] : le
//! programme écrit **toujours** une ligne non blanche et rend **toujours** le
//! code 0. Un payload vide, un JSON illisible, un assemblage qui panique : tous
//! ces chemins écrivent quelque chose. Se taire n'est jamais un affichage
//! minimal, c'est un effacement — Claude Code ne garde pas le texte précédent.

mod abonnement;
mod binaire;
mod cache;
mod capture;
mod conversions;
mod depot;
mod disque;
mod fenetres;
mod journal;
mod largeur;
mod modele;
mod reglages;
mod segments;
mod sortie;
mod temps;
mod usage;

use std::io::Read;

use serde_json::Value;

use crate::abonnement::lire_config;
use crate::capture::capturer;
use crate::conversions::{champ, en_texte, est_vrai};
use crate::fenetres::segment_fenetres;
use crate::journal::{journaliser, noter_panique};
use crate::largeur::largeur_console;
use crate::reglages::{MARGE_LARGEUR, MODELE_PAR_DEFAUT, RVB_CORPS, RVB_LIEU, RVB_TETE};
use crate::segments::{
    segment_abonnement, segment_contexte, segment_emplacement, segment_modele, segment_version,
};
use crate::sortie::{
    assembler_capsules, confirmer_ligne, ecrire_ligne, teinte_du_palier, Compartiment, Palier,
};

// ===========================================================================
// 7. Assemblage
// ===========================================================================

/// Extrait le nom du modèle. N'échoue jamais : le résultat sert aussi de repli
/// si l'assemblage échoue plus loin.
///
/// Repli sur un libellé neutre : le modèle est toujours présent en pratique,
/// mais la ligne doit rester lisible si le contrat d'entrée évolue.
fn nom_modele(donnees: &Value) -> String {
    let brut = champ(champ(donnees, "model"), "display_name");
    if est_vrai(brut) {
        if let Some(texte) = en_texte(brut) {
            return texte;
        }
    }
    MODELE_PAR_DEFAUT.to_string()
}

/// Assemble la sortie complète : quatre compartiments dans **une seule
/// capsule**, dépliée sur plusieurs rangs seulement quand la largeur l'impose.
/// Les segments absents se retirent d'eux-mêmes, un compartiment qui perd tous
/// les siens disparaît avec eux — jonction comprise —, et un rang qui perd tous
/// ses compartiments ne s'émet pas.
///
/// Les familles sont celles que la ligne décrivait déjà sans le dire : sous quel
/// régime (l'abonnement), ce qui tourne, où l'on est, ce que la session
/// consomme — cette dernière sur le fond du pire palier atteint, contexte,
/// fenêtres, projections et épuisement confondus. Jusqu'au 19/09/2026 elles
/// étaient quatre groupes joints par un espace ; ce sont désormais quatre
/// compartiments, chacun avec son fond — voir [`Compartiment`] et la forme de
/// la capsule dans [`crate::reglages`].
///
/// **Une seule rangée prévue, depuis le soir du 19/09/2026.** La capsule a
/// d'abord été livrée en deux lignes fixes — le régime, ce qui tourne et où
/// l'on est en haut, les mesures en bas — parce qu'une capsule unique
/// atteignait 134 colonnes à l'état critique. Puis le repli en largeur a rendu
/// la découpe fixe inutile : la capsule est mesurée contre la largeur de la
/// console, moins une marge, et ce qui ne tient pas passe au rang suivant à une
/// frontière propre, entre compartiments puis entre segments, sans rien perdre.
/// L'utilisateur a tranché aussitôt : **une ligne tant qu'elle tient**, un
/// second rang seulement quand c'est nécessaire — sur une console de 120
/// colonnes, la capsule entière tient presque toujours. Sans console à mesurer,
/// la capsule sort entière, sur une ligne.
///
/// **L'abonnement a pris son propre groupe le 26/08/2026 au soir**, en même
/// temps que sa pastille. Il ouvrait jusque-là le groupe « ce qui tourne », d'où
/// un « · » entre lui et la version ; un fond n'a pas besoin de ce point pour
/// qu'on voie où il s'arrête. La mécanique des compartiments lui donne le même
/// comportement lorsqu'il manque : la capsule commence alors à l'identité, cap
/// gauche en ardoise — Q4 du chantier.
///
/// La composition elle-même — jointure des segments, repli, jonctions, caps,
/// saut de ligne — vit dans [`crate::sortie`], où elle se teste sans toucher au
/// cache réel du poste : les producteurs de segments lisent le disque et
/// écrivent le cache, et un test qui passerait par l'assemblage complet
/// fausserait l'ancrage du rythme pour des heures.
fn assembler_ligne(donnees: &Value, modele: &str) -> String {
    // Une seule lecture de « ~/.claude.json » pour les deux segments qui s'y
    // servent : l'abonnement, et la fenêtre propre au modèle.
    let config = lire_config();

    // Les deux producteurs de mesures remontent leur palier ; le pire des deux
    // teinte le compartiment.
    let (contexte, palier_contexte) = match segment_contexte(donnees) {
        Some((texte, palier)) => (Some(texte), palier),
        None => (None, Palier::Aucun),
    };
    let (fenetres, palier_fenetres) = segment_fenetres(donnees, config.as_ref());
    let pire = palier_contexte.max(palier_fenetres);

    // Contexte puis fenêtres, un segment chacun : le compartiment les joint,
    // et le repli en largeur peut couper entre eux.
    let mut mesures = vec![contexte];
    mesures.extend(fenetres.into_iter().map(Some));

    let capsule = vec![
        Compartiment::nouveau(RVB_TETE, vec![segment_abonnement(config.as_ref())]),
        Compartiment::nouveau(
            RVB_CORPS,
            vec![
                segment_version(donnees),
                Some(segment_modele(donnees, modele)),
            ],
        ),
        Compartiment::nouveau(RVB_LIEU, vec![segment_emplacement(donnees)]),
        Compartiment::nouveau(teinte_du_palier(pire), mesures),
    ];

    // La sonde est lue une seconde fois par le journal, quelques microsecondes
    // plus tard : les deux lectures disent la même chose, et la garder ici
    // évite de faire passer une mesure d'affichage par le point de sortie.
    let capacite =
        largeur_console().map(|largeur| usize::from(largeur).saturating_sub(MARGE_LARGEUR));

    assembler_capsules(vec![capsule], capacite)
}

// ===========================================================================
// 8. Programme principal
// ===========================================================================
//
// Un seul invariant gouverne ce bloc : **toujours une ligne non blanche, et
// toujours le code de sortie 0**. Claude Code ne conserve pas le texte
// précédent — il pousse le résultat de chaque lancement tel quel, y compris
// vide, dès que la sortie est blanche ou que le code de sortie n'est pas nul.
// Une seule exécution muette efface donc la ligne, et rien ne la ramène avant
// le prochain rafraîchissement.

/// Écrit la ligne, journalise le lancement, relève le payload, et rend le
/// code 0.
///
/// Point de sortie unique : c'est ce qui garantit que **tous** les chemins du
/// programme principal — y compris les trois replis — écrivent quelque chose et
/// rendent 0. Les deux dispositifs de diagnostic viennent après l'écriture,
/// pour ne pas retarder l'affichage, et ne font rien tant que leur témoin est
/// absent.
///
/// `payload` est ce qui a été reçu sur l'entrée standard, en octets et tel
/// quel, transmis jusqu'ici pour que la capture atteigne **aussi** les chemins
/// de repli : un JSON que le programme n'a pas su lire est précisément celui
/// qu'on veut relever.
///
/// Les deux diagnostics tiennent sous leur propre filet depuis le 04/09/2026.
/// Ils viennent après l'écriture pour ne pas la retarder, mais une panique
/// dans l'un d'eux rendrait un code non nul, et Claude Code effacerait la
/// ligne pourtant écrite. Ils sont écrits sans rien qui panique ; le filet
/// fait que l'invariant ne repose plus sur cette seule discipline.
///
/// Le crabe `(V)°°(V)` était posé ici, et nulle part ailleurs, du 26/08/2026 au
/// soir du même jour : c'est le seul endroit que **tous** les chemins
/// traversent, si bien que la marque paraissait jusque sur une ligne réduite au
/// nom du modèle. L'abonnement a repris sa pastille mais pas cette place — il
/// est un segment, il s'assemble avec les autres et se retire comme eux. Une
/// ligne de repli n'a donc plus de signature, ce qui est le prix assumé de
/// rendre ces colonnes à ce qui informe.
fn terminer(debut: std::time::Instant, voie: &str, ligne: &str, payload: &[u8]) -> ! {
    ecrire_ligne(ligne);
    let _ = std::panic::catch_unwind(|| journaliser(debut.elapsed(), voie, ligne));
    let _ = std::panic::catch_unwind(|| capturer(payload));
    std::process::exit(0);
}

fn main() {
    let debut = std::time::Instant::now();

    // Un panic ne doit rien écrire sur la sortie d'erreur : Claude Code la
    // journalise, et le harnais de non-régression la capture avec la ligne.
    // Le message n'est pas perdu pour autant — 04/09/2026 : il est retenu pour
    // le journal, qui ne disait jusque-là que « panic », sans lieu ni cause.
    std::panic::set_hook(Box::new(|info| {
        noter_panique(info.location(), info.payload())
    }));

    // Lue en octets, convertie avec perte — 04/09/2026. Une entrée qui n'est
    // pas de l'UTF-8 valide faisait échouer la lecture entière, et la capture
    // manquait précisément le payload qu'on voudrait voir. Les octets sont
    // désormais relevés tels quels, et c'est l'analyse JSON qui tranche, sur
    // le texte converti.
    let mut octets = Vec::new();
    if std::io::stdin().read_to_end(&mut octets).is_err() {
        // Entrée illisible : il ne reste rien à afficher, mais se taire
        // coûterait la ligne. Le libellé neutre en tient la place. Ce qui a pu
        // être lu ne dit rien de sûr, donc rien à capturer — et le témoin
        // reste armé pour le tir suivant.
        terminer(debut, "stdin", MODELE_PAR_DEFAUT, &[]);
    }
    let brut = String::from_utf8_lossy(&octets);

    // Payload absent ou blanc. Le cas ne devrait pas se produire — Claude Code
    // écrit le JSON sur l'entrée standard avant de la fermer — mais cette
    // écriture peut échouer sans que le programme en sache rien : le tube rompu
    // est signalé à l'appelant, pas au processus lancé, qui ne voit qu'une
    // entrée vide.
    if brut.trim().is_empty() {
        terminer(debut, "vide", MODELE_PAR_DEFAUT, &octets);
    }

    let donnees: Value = match serde_json::from_str(&brut) {
        Ok(v) => v,
        Err(_) => terminer(debut, "json", MODELE_PAR_DEFAUT, &octets),
    };

    // Le modèle est résolu à part, avant tout ce qui peut échouer : il tient
    // lieu de repli si l'assemblage panique.
    let modele = std::panic::catch_unwind(|| nom_modele(&donnees))
        .unwrap_or_else(|_| MODELE_PAR_DEFAUT.to_string());

    // La voie distingue au journal une ligne courte — segments retirés faute de
    // données — d'une ligne dégradée par un panic.
    let (ligne, voie) = match std::panic::catch_unwind(|| assembler_ligne(&donnees, &modele)) {
        Ok(assemblee) => (assemblee, "ok"),
        Err(_) => (String::new(), "panic"),
    };

    terminer(debut, voie, &confirmer_ligne(&ligne, &modele), &octets);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le nom du modèle se lit dans le payload, et tombe sur le libellé neutre
    /// quand il manque ou n'est pas une chaîne : c'est lui qui tient lieu de
    /// repli si l'assemblage échoue.
    #[test]
    fn le_nom_du_modele_a_un_repli() {
        let donnees: Value = serde_json::json!({ "model": { "display_name": "Opus 5" } });
        assert_eq!(nom_modele(&donnees), "Opus 5");
        assert_eq!(nom_modele(&serde_json::json!({})), MODELE_PAR_DEFAUT);
        assert_eq!(
            nom_modele(&serde_json::json!({ "model": { "display_name": "" } })),
            MODELE_PAR_DEFAUT
        );
        // Un nombre est rendu en texte plutôt que rejeté : c'est la lecture
        // défensive d'`en_texte`, qui préfère une valeur inattendue à un repli.
        assert_eq!(
            nom_modele(&serde_json::json!({ "model": { "display_name": 12 } })),
            "12"
        );
    }

    /// Verrou de non-retour : la barre `│` a coûté six colonnes pour encadrer
    /// une pastille qui se bornait elle-même, et rien dans le code ne
    /// rappellerait autrement pourquoi elle a disparu le 26/08/2026 au soir. La
    /// capsule sépare ses compartiments par la forme, jamais par un trait.
    #[test]
    fn aucune_barre_de_groupe_dans_la_forme() {
        use crate::reglages::{CAP_DROIT, CAP_GAUCHE, JONCTION, LISERE, SEPARATEUR};
        for glyphe in [CAP_GAUCHE, CAP_DROIT, JONCTION, LISERE, SEPARATEUR] {
            assert!(!glyphe.contains('│'), "la barre de groupe est revenue");
        }
    }
}
