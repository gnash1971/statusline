//! Segments de la ligne : abonnement, version, modèle, emplacement, contexte.
//!
//! Reste de la section 6, une fois les fenêtres de limitation parties dans
//! [`crate::fenetres`]. Ces cinq-là ont en commun de ne faire que mettre en
//! forme ce qu'ils trouvent — aucun ne mesure, aucun n'écrit sur le disque — et
//! de savoir se retirer quand la donnée manque : c'est ce qui garde la ligne
//! honnête plutôt que remplie.

use serde_json::Value;

use crate::abonnement::abonnement;
use crate::binaire::version_binaire;
use crate::conversions::{
    arrondir, champ, convertir_nombre, convertir_pourcent, en_texte, est_vrai,
};
use crate::depot::branche_git;
use crate::reglages::{
    CODE_TEXTE_LIEU, CODE_TEXTE_LIEU_SOURD, CODE_TEXTE_TETE, FENETRE_CONTEXTE_ORDINAIRE,
    LIBELLE_MODE_RAPIDE, LIBELLE_SANS_REFLEXION, PREFIXE_VERSION, PROFONDEUR_MAX_CHEMIN,
    SEUIL_CONTEXTE, SEUIL_CRITIQUE_CONTEXTE, UNITE_POURCENT,
};
use crate::sortie::{attenuer, colorer, formater_mesure, marquer, texte_plein, Palier};

/// Replie une suite de segments de chemin en « racine\…\feuille » au-delà de la
/// profondeur maximale, pour ne pas manger la ligne.
fn compresser_chemin(segments: &[String]) -> Vec<String> {
    if segments.len() <= PROFONDEUR_MAX_CHEMIN {
        return segments.to_vec();
    }
    vec![
        segments[0].clone(),
        "…".to_string(),
        segments[segments.len() - 1].clone(),
    ]
}

/// Dernier segment d'un chemin, comme `Split-Path -Leaf`.
fn feuille(chemin: &str) -> String {
    match chemin.rfind(['\\', '/']) {
        Some(pos) => chemin[pos + 1..].to_string(),
        None => chemin.to_string(),
    }
}

/// Rend l'offset, en octets de `texte`, qui suit un préfixe comparé sans la
/// casse, ou `None` si `texte` ne commence pas par lui.
///
/// La comparaison se fait caractère par caractère — voir [`meme_lettre`] —, et
/// l'offset est pris dans `texte` : il tombe donc toujours sur une frontière
/// de caractère, quelle que soit la lettre. Comparer deux copies entièrement
/// mises en minuscules puis couper l'original à la longueur du préfixe ne
/// l'était pas — 04/09/2026 : le signe Kelvin « K », trois octets, y devenait
/// « k », un seul, et la coupe tombait au mauvais endroit du chemin —
/// « kelvin\n\src » pour « kelvin\src » —, voire au milieu d'un caractère.
fn apres_prefixe_sans_casse(texte: &str, prefixe: &str) -> Option<usize> {
    let mut fin = 0;
    let mut indices = texte.char_indices();
    for attendu in prefixe.chars() {
        let (debut, trouve) = indices.next()?;
        if !meme_lettre(trouve, attendu) {
            return None;
        }
        fin = debut + trouve.len_utf8();
    }
    Some(fin)
}

/// Indique si deux caractères ne diffèrent que par la casse, au sens de
/// `StringComparison.OrdinalIgnoreCase`, que suit l'oracle.
///
/// Deux lettres se valent si leurs minuscules **et** leurs majuscules
/// concordent. C'est ce qui reproduit la table de .NET, qui écarte les
/// correspondances non réversibles — relevé sur le poste le 04/09/2026 : le s
/// long « ſ », le signe Kelvin « K », le İ et le ı turcs, le signe Ohm « Ω »
/// et le « ẞ » n'y valent pas leur homologue, quand « Ⱥ » et « ⱥ », ou « É » et
/// « é », se valent. Une seule des deux correspondances, en minuscules comme en
/// majuscules, en laisserait passer une moitié et refuserait l'autre.
fn meme_lettre(a: char, b: char) -> bool {
    a == b || (a.to_lowercase().eq(b.to_lowercase()) && a.to_uppercase().eq(b.to_uppercase()))
}

/// Met en forme le répertoire de travail, relativement au répertoire de
/// lancement de la session.
///
/// La feuille seule ne suffit pas sur un workspace multi-projets : « tests » ou
/// « src » ne disent pas d'où l'on parle. Le chemin est donc affiché depuis la
/// racine du projet. Hors du projet, la feuille seule fait l'affaire.
///
/// La comparaison est insensible à la casse, en Unicode : sous Windows deux
/// écritures du même chemin ne diffèrent souvent que par la casse, et rien ne
/// garantit que Claude Code renvoie `current_dir` et `project_dir` dans la
/// même.
fn formater_repertoire(courant: Option<&str>, projet: Option<&str>) -> Option<String> {
    let courant = courant?;
    if courant.trim().is_empty() {
        return None;
    }

    let courant_net = courant.trim_end_matches(['\\', '/']);
    let feuille_courante = feuille(courant_net);

    let projet = match projet {
        Some(p) if !p.trim().is_empty() => p,
        _ => return Some(feuille_courante),
    };
    let projet_net = projet.trim_end_matches(['\\', '/']);

    // Même chemin, à la casse près : le préfixe couvre `courant_net` en entier.
    if apres_prefixe_sans_casse(courant_net, projet_net) == Some(courant_net.len()) {
        return Some(feuille(projet_net));
    }

    let prefixe = format!("{}\\", projet_net);
    let Some(debut_relatif) = apres_prefixe_sans_casse(courant_net, &prefixe) else {
        return Some(feuille_courante);
    };

    let relatif = &courant_net[debut_relatif..];
    let mut segments = vec![feuille(projet_net)];
    segments.extend(
        relatif
            .split(['\\', '/'])
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
    );

    Some(compresser_chemin(&segments).join("\\"))
}

/// Produit le segment d'emplacement : répertoire, et branche entre parenthèses
/// quand il y en a une — **hiérarchisé** depuis le 19/09/2026.
///
/// Sur un workspace multi-projets, savoir d'où l'on parle prime sur tout le
/// reste, et c'est la seule information de la ligne que l'œil doive retrouver
/// sans la chercher. Du 21/08 au 19/09/2026 c'est ce segment qui portait la
/// pastille, seul fond de la ligne ; il occupe désormais son propre
/// compartiment de la capsule, en sauge, et le fond n'est plus posé ici mais
/// par l'assemblage — voir [`RVB_LIEU`](crate::reglages::RVB_LIEU).
///
/// Le segment ne rend donc que des avant-plans, en deux teintes : les ancêtres
/// du chemin et les parenthèses de la branche en sourd, ce qui situe ; la
/// feuille et la branche en clair, ce qui distingue le sous-projet. Sous
/// `NO_COLOR`, le texte est celui d'avant : `répertoire (branche)`.
pub(crate) fn segment_emplacement(donnees: &Value) -> Option<String> {
    // « cwd » double « workspace.current_dir » et sert de repli si le contrat
    // d'entrée venait à ne plus porter l'objet « workspace ».
    let brut = champ(champ(donnees, "workspace"), "current_dir");
    let courant = if est_vrai(brut) {
        en_texte(brut)
    } else {
        en_texte(champ(donnees, "cwd"))
    };

    let projet = en_texte(champ(champ(donnees, "workspace"), "project_dir"));
    let repertoire = formater_repertoire(courant.as_deref(), projet.as_deref())?;
    if repertoire.is_empty() {
        return None;
    }

    // « worktree.branch » n'existe que pour les sessions --worktree ; partout
    // ailleurs la branche se lit sur le disque.
    let branche_payload = champ(champ(donnees, "worktree"), "branch");
    let branche = if est_vrai(branche_payload) {
        en_texte(branche_payload)
    } else {
        courant.as_deref().and_then(branche_git)
    };

    // Les segments du chemin ne portent jamais de « \ » — ils viennent d'une
    // coupe sur ce caractère — : le dernier antislash sépare donc les ancêtres,
    // repli « … » compris, de la feuille. Hors du projet, la feuille seule.
    let (ancetres, feuille) = match repertoire.rsplit_once('\\') {
        Some((tete, feuille)) => (format!("{}\\", tete), feuille.to_string()),
        None => (String::new(), repertoire.clone()),
    };

    let mut emplacement = String::new();
    if !ancetres.is_empty() {
        emplacement.push_str(&colorer(&ancetres, CODE_TEXTE_LIEU_SOURD));
    }
    emplacement.push_str(&colorer(&feuille, CODE_TEXTE_LIEU));

    // Les parenthèses sont du chrome, la branche une information : elle prend
    // le clair de la feuille, elles le sourd des ancêtres.
    if let Some(b) = branche.filter(|b| !b.is_empty()) {
        emplacement.push_str(&colorer(" (", CODE_TEXTE_LIEU_SOURD));
        emplacement.push_str(&colorer(&b, CODE_TEXTE_LIEU));
        emplacement.push_str(&colorer(")", CODE_TEXTE_LIEU_SOURD));
    }

    Some(emplacement)
}

/// Produit le segment d'abonnement, tout en tête de ligne : le texte du
/// compartiment de **tête** de la capsule, en brun sur la teinte de marque.
///
/// Il ouvre la ligne parce qu'il en est le cadre : la version dit quel Claude
/// Code, le modèle lequel de ses modèles, et l'abonnement sous quel régime les
/// deux tournent — c'est lui qui décide de la taille des fenêtres de limitation
/// affichées à l'autre bout.
///
/// # De l'atténuation à la pastille, puis à la tête de capsule
///
/// Il a été atténué comme la version, et pour la même raison : c'est
/// l'information la moins volatile de toute la ligne, celle qui se consulte et
/// ne se surveille pas. Le 26/08/2026 au soir il a pris la **pastille de tête**,
/// celle que le crabe `(V)°°(V)` avait portée quelques heures — le logo prenait
/// huit colonnes pour ne rien dire de la session, l'abonnement en prend trois
/// et dit le régime sous lequel elle tourne. La marque du produit restait,
/// c'est sa teinte qui peignait le texte de la pastille, sur un fond `#300E00`
/// que l'écran ne distinguait pas du terminal.
///
/// Depuis le 19/09/2026 la teinte de marque est le **fond** du compartiment de
/// tête, et le texte est brun : la marque est devenue une forme. Le fond n'est
/// plus posé ici mais par l'assemblage — voir
/// [`RVB_TETE`](crate::reglages::RVB_TETE) — ; ce segment ne rend que le texte
/// et sa couleur.
///
/// Le logo était posé après [`crate::confirmer_ligne`] et paraissait donc **sur
/// les lignes de repli**, y compris quand l'assemblage avait échoué.
/// L'abonnement, lui, est un segment : sur un payload illisible, la ligne se
/// réduit au nom du modèle, sans capsule. C'est cohérent avec ce qu'il est —
/// une information, pas une marque —, mais la ligne dégradée n'a rien qui la
/// signe, et Q4 du chantier a confirmé qu'aucun compartiment vide ne viendrait
/// la signer à sa place.
///
/// La valeur est lue sur le disque, le payload n'en portant rien : voir
/// [`crate::abonnement`], qui porte aussi la raison du fichier choisi. La
/// configuration arrive déjà analysée, une seule lecture servant deux
/// segments depuis le soir du 03/09/2026.
pub(crate) fn segment_abonnement(config: Option<&Value>) -> Option<String> {
    let abonnement = abonnement(config)?;
    if abonnement.trim().is_empty() {
        return None;
    }

    Some(colorer(&abonnement, CODE_TEXTE_TETE))
}

/// Indique si le binaire posé sur le disque a été remplacé sous la session, qui
/// tourne alors encore sur l'ancien.
///
/// Extraite de [`segment_version`] pour rester vérifiable : la fonction
/// complète lit le disque du poste, quand la décision, elle, ne dépend que de
/// deux chaînes. C'est le mouvement de `colorer_selon` et `pastiller_selon`.
///
/// L'écart demande **deux** valeurs connues. Une seule, ou aucune, n'est pas un
/// écart mais une inconnue : teinter là-dessus annoncerait une mise à jour dont
/// rien ne dit qu'elle existe.
fn version_decalee(du_disque: Option<&str>, de_la_session: Option<&str>) -> bool {
    match (du_disque, de_la_session) {
        (Some(disque), Some(session)) => disque.trim() != session.trim(),
        _ => false,
    }
}

/// Produit le segment de version, en tête de ligne.
///
/// La version affichée est celle du binaire posé sur le disque, et non celle
/// que la session a chargée en mémoire : une mise à jour automatique remplace
/// le binaire sans toucher au processus en cours, et l'écart entre les deux
/// dure jusqu'au prochain lancement. C'est précisément ce que ce segment sert à
/// voir.
///
/// Repli sur « version » du payload quand le binaire est introuvable : la
/// version de la session reste une réponse honnête à « quelle version de Claude
/// Code ». Le repli exige une chaîne — un payload dégénéré ne doit pas se
/// retrouver mis en forme.
///
/// # La teinte dit l'écart — 26/08/2026
///
/// Le segment coûtait onze colonnes permanentes pour dire une chose qui n'arrive
/// qu'à une mise à jour près. Il porte donc désormais la teinte des marqueurs,
/// [`marquer`], **quand les deux versions diffèrent**, et son gris de chrome le
/// reste du temps : la ligne signale l'écart au lieu de le laisser à qui pense à
/// comparer deux nombres de quatre chiffres. C'est la règle que tiennent déjà
/// les marqueurs de mode — ne rien dire de l'ordinaire, se voir sur l'écart.
///
/// La comparaison exige les **deux** valeurs. Un binaire introuvable ou un
/// payload muet ne produit donc aucune teinte : il n'y a pas d'écart constaté,
/// seulement une inconnue, et le cyan annoncerait à tort une mise à jour en
/// attente.
pub(crate) fn segment_version(donnees: &Value) -> Option<String> {
    let du_disque = version_binaire();
    let de_la_session = match champ(donnees, "version") {
        Value::String(s) => Some(s.clone()),
        _ => None,
    };

    let version = du_disque
        .clone()
        .or_else(|| de_la_session.clone())
        .filter(|v| !v.trim().is_empty())?;

    let decale = version_decalee(du_disque.as_deref(), de_la_session.as_deref());
    let texte = format!("{}{}", PREFIXE_VERSION, version);

    // Atténué le reste du temps, comme tout le chrome : c'est l'information la
    // moins volatile de la ligne, celle qu'on consulte de loin en loin plutôt
    // qu'on ne surveille. Elle a porté la pastille du 21/08/2026 au 22/08/2026,
    // le temps de constater que le fond servait mieux l'emplacement — voir
    // [`segment_emplacement`].
    Some(if decale {
        marquer(&texte)
    } else {
        attenuer(&texte)
    })
}

/// Met en forme un nombre de jetons en « 1M » ou « 500k ».
///
/// L'arrondi est franc — au million au-delà d'un million, au millier en deçà —
/// parce qu'aucune fenêtre réelle ne tombe entre deux : les tailles annoncées
/// par Claude Code sont des nombres ronds, et une décimale n'apprendrait rien
/// tout en allongeant le seul segment que l'œil lit en premier.
fn formater_jetons(jetons: f64) -> String {
    if jetons >= 1_000_000.0 {
        format!("{}M", arrondir(jetons / 1_000_000.0) as i64)
    } else {
        format!("{}k", arrondir(jetons / 1_000.0) as i64)
    }
}

/// Rend le marqueur de fenêtre de contexte, ou `None` quand elle est ordinaire.
///
/// Le marqueur ne dit pas « ce modèle peut faire 1M », mais « cette session-ci
/// tourne en 1M » : `context_window_size` est la fenêtre **effective**, celle
/// sur laquelle Claude Code calcule le pourcentage affiché juste après. C'est
/// ce qui en fait le bon champ à lire — plutôt que `exceeds_200k_tokens`, qui
/// est la cause du basculement et non son résultat, et qui ne dirait rien de la
/// taille atteinte.
///
/// Il mérite sa place pour la même raison : le basculement en contexte étendu
/// divise le pourcentage affiché par cinq d'un rafraîchissement à l'autre, sans
/// que rien n'ait été libéré. Sans marqueur, cette chute se lit comme un
/// compactage, et « ctx 12% » ne dit plus 12 % de quoi.
///
/// Une garde évite le doublon : si le nom du modèle porte déjà la mention — ce
/// que Claude Code ne fait pas aujourd'hui, son suffixe `[1m]` restant du côté
/// de `model.id` — le marqueur se retire, plutôt que d'écrire « Opus 5 1M 1M ».
fn marqueur_fenetre(donnees: &Value, modele: &str) -> Option<String> {
    let contexte = champ(donnees, "context_window");
    let taille = convertir_nombre(champ(contexte, "context_window_size"))?;

    if taille <= FENETRE_CONTEXTE_ORDINAIRE {
        return None;
    }

    let marqueur = formater_jetons(taille);
    if modele.to_lowercase().contains(&marqueur.to_lowercase()) {
        return None;
    }

    Some(marqueur)
}

/// Produit le segment de modèle : nom, fenêtre de contexte, effort de
/// raisonnement, puis marqueurs de mode. Seul segment à ne jamais rendre
/// `None`, `modele` portant déjà un repli.
///
/// L'effort est accolé au nom plutôt que posé en segment propre : il qualifie
/// le modèle. Il mérite sa place parce qu'il se règle à trois endroits —
/// settings, drapeau de lancement, « /effort » en cours de session — dont le
/// dernier ne laisse aucune trace ailleurs dans l'interface, et parce que c'est
/// lui qui gouverne la vitesse à laquelle se remplissent les fenêtres affichées
/// plus loin.
///
/// Les marqueurs ne s'affichent que sur l'état inhabituel — contexte étendu,
/// mode rapide actif, réflexion étendue coupée — dans le même esprit que les
/// segments qui se retirent : la ligne ne porte que ce qui s'écarte de
/// l'ordinaire.
///
/// Ils portent depuis le 25/08/2026 une **teinte propre**, [`marquer`], au lieu
/// de la couleur pleine du thème qui était aussi celle du nom du modèle : « Opus
/// 5 1M » se lisait comme un seul nom, alors que la moitié droite décrit la
/// session et non le modèle.
///
/// L'ordre n'est pas indifférent. Le marqueur de fenêtre vient **avant**
/// l'effort, collé au nom, parce qu'il dit quel modèle tourne — sa fenêtre fait
/// partie de son identité, et Claude Code l'exprime lui-même ainsi en
/// suffixant `model.id` de `[1m]`. L'effort et les deux marqueurs de mode
/// décrivent au contraire la conduite de la session, et restent groupés
/// derrière.
///
/// Les trois champs booléens sont contrôlés en type avant lecture, comme dans
/// le script d'origine où un `-eq` appliqué à un tableau filtre au lieu de
/// comparer.
pub(crate) fn segment_modele(donnees: &Value, modele: &str) -> String {
    // En texte plein explicite depuis la capsule : la couleur par défaut du
    // terminal n'est plus celle du fond sur lequel le nom s'écrit.
    let mut morceaux = vec![texte_plein(modele)];

    // Dans la teinte des marqueurs, comme les deux marqueurs de mode : il ne
    // paraît que sur l'état inhabituel, et l'atténuer irait contre la raison de
    // sa présence.
    if let Some(marqueur) = marqueur_fenetre(donnees, modele) {
        morceaux.push(marquer(&marqueur));
    }

    // L'effort est atténué : il qualifie le modèle et se consulte, là où le nom
    // du modèle identifie la session. Les deux marqueurs qui suivent prennent la
    // teinte des marqueurs — ils ne s'affichent que sur l'état inhabituel.
    if let Value::String(effort) = champ(champ(donnees, "effort"), "level") {
        if !effort.trim().is_empty() {
            morceaux.push(attenuer(effort.trim()));
        }
    }

    if let Value::Bool(true) = champ(donnees, "fast_mode") {
        morceaux.push(marquer(LIBELLE_MODE_RAPIDE));
    }

    if let Value::Bool(false) = champ(champ(donnees, "thinking"), "enabled") {
        morceaux.push(marquer(LIBELLE_SANS_REFLEXION));
    }

    morceaux.join(" ")
}

/// Produit le segment d'occupation du contexte, avec le palier qu'il atteint —
/// l'assemblage en teinte le compartiment des mesures.
///
/// `used_percentage` est nul avant le premier appel API, et de nouveau après un
/// /compact tant que rien n'a été renvoyé : le segment disparaît alors, plutôt
/// que d'afficher un « ctx 0% » qui se lirait comme une mesure.
pub(crate) fn segment_contexte(donnees: &Value) -> Option<(String, Palier)> {
    let contexte = champ(donnees, "context_window");
    if contexte.is_null() {
        return None;
    }

    let pourcent = convertir_pourcent(champ(contexte, "used_percentage"))?;

    Some(formater_mesure(
        "ctx",
        &pourcent.to_string(),
        UNITE_POURCENT,
        pourcent,
        SEUIL_CONTEXTE,
        SEUIL_CRITIQUE_CONTEXTE,
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// L'emplacement distingue la feuille de ses ancêtres, et la branche de
    /// ses parenthèses ; il ne pose aucun fond, c'est le compartiment qui le
    /// porte.
    ///
    /// Les attendus passent par [`colorer`] plutôt que par une séquence écrite
    /// en dur : les deux côtés suivent ainsi la même décision `NO_COLOR`, que
    /// Claude Code définit justement dans l'environnement des sous-processus
    /// qu'il lance — `cargo test` compris.
    #[test]
    fn l_emplacement_distingue_la_feuille_des_ancetres() {
        let clair = |t: &str| colorer(t, CODE_TEXTE_LIEU);
        let sourd = |t: &str| colorer(t, CODE_TEXTE_LIEU_SOURD);

        // Chemin volontairement inexistant : la recherche de branche s'arrête
        // aussitôt, sans remonter à la recherche d'un « .git », et le test ne
        // dépend donc pas de l'arborescence du poste. Hors projet : la feuille
        // seule, en clair.
        let hors_projet = json!({
            "workspace": { "current_dir": "C:\\aucun-dossier-de-ce-nom-4c1f\\PY_xl" }
        });
        assert_eq!(segment_emplacement(&hors_projet), Some(clair("PY_xl")));

        // Dans le projet : les ancêtres en sourd, antislash compris, la
        // feuille en clair.
        let dans_projet = json!({
            "workspace": {
                "current_dir": "C:\\aucun-dossier-de-ce-nom-4c1f\\PY_xl\\PyScripts\\_dsn_",
                "project_dir": "C:\\aucun-dossier-de-ce-nom-4c1f\\PY_xl"
            }
        });
        assert_eq!(
            segment_emplacement(&dans_projet),
            Some(format!("{}{}", sourd("PY_xl\\PyScripts\\"), clair("_dsn_")))
        );

        // Replié : le « … » reste parmi les ancêtres.
        let replie = json!({
            "workspace": {
                "current_dir": "C:\\aucun-dossier-de-ce-nom-4c1f\\PY_xl\\a\\b\\c",
                "project_dir": "C:\\aucun-dossier-de-ce-nom-4c1f\\PY_xl"
            }
        });
        assert_eq!(
            segment_emplacement(&replie),
            Some(format!("{}{}", sourd("PY_xl\\…\\"), clair("c")))
        );

        // Branche portée par le payload d'une session --worktree : en clair,
        // entre des parenthèses sourdes.
        let avec_branche = json!({
            "workspace": { "current_dir": "C:\\aucun-dossier-de-ce-nom-4c1f\\dsn" },
            "worktree": { "branch": "main" }
        });
        assert_eq!(
            segment_emplacement(&avec_branche),
            Some(format!(
                "{}{}{}{}",
                clair("dsn"),
                sourd(" ("),
                clair("main"),
                sourd(")")
            ))
        );

        // Aucune séquence de fond : le compartiment s'en charge.
        assert!(!segment_emplacement(&dans_projet)
            .unwrap_or_default()
            .contains("[48;"));
    }

    /// Le contexte remonte son palier avec sa mise en forme.
    #[test]
    fn le_contexte_remonte_son_palier() {
        let a = |p: i64| json!({ "context_window": { "used_percentage": p } });
        assert_eq!(
            segment_contexte(&a(34)).map(|(_, p)| p),
            Some(Palier::Aucun)
        );
        assert_eq!(
            segment_contexte(&a(80)).map(|(_, p)| p),
            Some(Palier::Alerte)
        );
        assert_eq!(
            segment_contexte(&a(92)).map(|(_, p)| p),
            Some(Palier::Critique)
        );
        // Sans mesure, pas de segment — et donc pas de palier à remonter.
        assert!(segment_contexte(&json!({ "context_window": {} })).is_none());
        assert!(segment_contexte(&json!({})).is_none());
    }

    /// Fabrique un payload ne portant que la taille de fenêtre voulue.
    fn payload_fenetre(taille: Value) -> Value {
        json!({ "context_window": { "context_window_size": taille } })
    }

    #[test]
    fn marqueur_de_fenetre_sur_le_contexte_etendu_seulement() {
        // La fenêtre ordinaire ne se signale pas : elle est l'implicite.
        assert_eq!(
            marqueur_fenetre(&payload_fenetre(json!(200_000)), "Opus 5"),
            None
        );
        assert_eq!(
            marqueur_fenetre(&payload_fenetre(json!(1_000_000)), "Opus 5"),
            Some("1M".to_string())
        );
        // Une fenêtre intermédiaire se dit en milliers, sans supposer le
        // million.
        assert_eq!(
            marqueur_fenetre(&payload_fenetre(json!(500_000)), "Opus 5"),
            Some("500k".to_string())
        );
        // Champ absent, ou valeur inexploitable : le marqueur se retire, comme
        // tout ce qui ne repose pas sur une donnée sûre.
        assert_eq!(marqueur_fenetre(&json!({}), "Opus 5"), None);
        assert_eq!(
            marqueur_fenetre(&payload_fenetre(json!("beaucoup")), "Opus 5"),
            None
        );
        // Garde anti-doublon : si le nom porte déjà la mention, ne pas la
        // répéter.
        assert_eq!(
            marqueur_fenetre(&payload_fenetre(json!(1_000_000)), "Opus 5 (1M)"),
            None
        );
    }

    #[test]
    fn marqueur_de_fenetre_precede_l_effort() {
        let donnees = json!({
            "context_window": { "context_window_size": 1_000_000 },
            "effort": { "level": "xhigh" },
            "fast_mode": true
        });

        // La fenêtre est collée au nom du modèle ; l'effort et les marqueurs de
        // conduite suivent. Trois teintes se succèdent : le nom en texte plein,
        // la fenêtre et le mode rapide en marqueurs, l'effort en chrome.
        assert_eq!(
            segment_modele(&donnees, "Opus 5"),
            format!(
                "{} {} {} {}",
                texte_plein("Opus 5"),
                marquer("1M"),
                attenuer("xhigh"),
                marquer("fast")
            )
        );
    }

    #[test]
    fn la_version_ne_se_teinte_que_sur_un_ecart_constate() {
        // Le binaire du disque a été remplacé sous la session : c'est le seul
        // cas où le segment sort de son gris.
        assert!(version_decalee(Some("2.1.247"), Some("2.1.246")));
        // Les deux concordent : rien à signaler.
        assert!(!version_decalee(Some("2.1.246"), Some("2.1.246")));
        // Les blancs encadrants ne font pas un écart.
        assert!(!version_decalee(Some(" 2.1.246 "), Some("2.1.246")));
        // Une seule valeur, ou aucune : une inconnue n'est pas un écart.
        assert!(!version_decalee(Some("2.1.246"), None));
        assert!(!version_decalee(None, Some("2.1.246")));
        assert!(!version_decalee(None, None));
    }

    #[test]
    fn repertoire_relatif_au_projet() {
        let projet = Some("C:\\projet");
        assert_eq!(
            formater_repertoire(Some("C:\\projet"), projet).as_deref(),
            Some("projet")
        );
        assert_eq!(
            formater_repertoire(Some("C:\\projet\\src"), projet).as_deref(),
            Some("projet\\src")
        );
        // Au-delà de trois segments, le milieu se replie.
        assert_eq!(
            formater_repertoire(Some("C:\\projet\\src\\api\\v2"), projet).as_deref(),
            Some("projet\\…\\v2")
        );
        // Hors du projet, la feuille seule.
        assert_eq!(
            formater_repertoire(Some("C:\\ailleurs\\bin"), projet).as_deref(),
            Some("bin")
        );
        // Casse différente : le chemin reste relatif au projet.
        assert_eq!(
            formater_repertoire(Some("C:\\PROJET\\SRC"), projet).as_deref(),
            Some("projet\\SRC")
        );
    }

    /// Casse à longueur d'octets variable : « Ⱥ » fait deux octets là où « ⱥ »
    /// en fait trois, et les deux se valent. La coupe suit les caractères, pas
    /// la longueur du préfixe — sans quoi elle tomberait un octet trop loin,
    /// voire au milieu d'un caractère.
    #[test]
    fn le_prefixe_se_compare_caractere_par_caractere() {
        let arbre = Some("C:\\\u{2C65}rbre");
        assert_eq!(
            formater_repertoire(Some("C:\\\u{23A}rbre\\src"), arbre).as_deref(),
            Some("\u{2C65}rbre\\src")
        );
        assert_eq!(
            formater_repertoire(Some("C:\\\u{23A}rbre"), arbre).as_deref(),
            Some("\u{2C65}rbre")
        );
        // Préfixe seulement partagé : hors projet, la feuille seule.
        assert_eq!(
            formater_repertoire(Some("C:\\\u{2C65}rbrex\\bin"), arbre).as_deref(),
            Some("bin")
        );
        // Le signe Kelvin « K » n'est pas un « k », ni le s long « ſ » un « s »,
        // pour l'oracle : un autre chemin, donc la feuille seule.
        assert_eq!(
            formater_repertoire(Some("C:\\\u{212A}elvin\\src"), Some("C:\\kelvin")).as_deref(),
            Some("src")
        );
        assert_eq!(
            formater_repertoire(Some("C:\\\u{17F}tuff\\src"), Some("C:\\stuff")).as_deref(),
            Some("src")
        );

        // Le cœur, à nu : l'offset rendu est une frontière de `texte`.
        assert_eq!(
            apres_prefixe_sans_casse("\u{23A}rbre\\src", "\u{2C65}RBRE\\"),
            Some(7)
        );
        assert_eq!(apres_prefixe_sans_casse("kelvin", "KELVIN"), Some(6));
        assert_eq!(apres_prefixe_sans_casse("kel", "kelvin"), None);
        assert_eq!(apres_prefixe_sans_casse("kelvin", ""), Some(0));
    }

    /// Les neuf réponses d'`OrdinalIgnoreCase`, relevées sur le poste le
    /// 04/09/2026 : les correspondances non réversibles ne valent pas.
    #[test]
    fn meme_lettre_suit_ordinal_ignore_case() {
        assert!(meme_lettre('a', 'A'));
        assert!(meme_lettre('\u{C9}', '\u{E9}'));
        assert!(meme_lettre('\u{23A}', '\u{2C65}'));
        assert!(meme_lettre('\u{23E}', '\u{2C66}'));
        assert!(!meme_lettre('\u{17F}', 's'));
        assert!(!meme_lettre('\u{212A}', 'k'));
        assert!(!meme_lettre('\u{130}', 'i'));
        assert!(!meme_lettre('\u{131}', 'i'));
        assert!(!meme_lettre('\u{2126}', '\u{3C9}'));
        assert!(!meme_lettre('\u{1E9E}', '\u{DF}'));
    }
}
