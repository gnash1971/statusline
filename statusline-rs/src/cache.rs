//! État sur disque : cache des fenêtres — section 4 du script d'origine.
//!
//! Le cache existe parce que Claude Code ne renseigne `rate_limits` qu'à partir
//! de la première requête de la session. Sa forme est reproduite octet pour
//! octet d'une écriture à l'autre : c'est ce que compare le harnais de
//! non-régression, et c'est pourquoi la sérialisation est écrite à la main
//! plutôt que confiée à `serde`.

use serde_json::Value;

use crate::conversions::{
    arrondir, champ, convertir_instant, convertir_nombre, convertir_pourcent,
};
use crate::disque::{chemin_etat, ecrire_atomique, lire_texte, lire_texte_resultat};
use crate::reglages::{ATTENTE_RELECTURE_CACHE_MS, NOM_CACHE, TENTATIVES_LECTURE_CACHE};

/// Chemin du fichier de cache, ou `None` si aucune racine n'est disponible.
fn chemin_cache() -> Option<std::path::PathBuf> {
    chemin_etat(NOM_CACHE)
}

/// Indique si un échec de lecture du cache mérite un nouvel essai.
///
/// Deux cas seulement ne le méritent pas : le fichier est **absent**, ce qui est
/// définitif et attendu au premier lancement d'une session, ou les tentatives
/// sont épuisées. Tout le reste — accès refusé le temps d'un remplacement
/// concurrent, lecture interrompue — est tenu pour passager.
fn reessayer(erreur: &std::io::Error, tentative: u32) -> bool {
    erreur.kind() != std::io::ErrorKind::NotFound && tentative < TENTATIVES_LECTURE_CACHE
}

/// Relit le cache des fenêtres à un emplacement donné, ou `None`.
///
/// Séparée de [`lire_cache`] pour rester vérifiable sans toucher à
/// `%LOCALAPPDATA%` : les tests s'exécutent en parallèle, et une variable
/// d'environnement détournée le temps d'un test contaminerait les autres.
///
/// Un contenu présent mais invalide n'est **pas** réessayé : l'écriture passant
/// par un fichier temporaire renommé, on ne peut jamais lire un JSON à moitié
/// écrit, et ce qui est mal formé le restera au tir suivant.
fn lire_cache_a(chemin: &std::path::Path) -> Option<Value> {
    for tentative in 1..=TENTATIVES_LECTURE_CACHE {
        match lire_texte_resultat(chemin) {
            Ok(texte) => return serde_json::from_str(&texte).ok(),
            Err(erreur) if reessayer(&erreur, tentative) => {
                std::thread::sleep(std::time::Duration::from_millis(ATTENTE_RELECTURE_CACHE_MS));
            }
            Err(_) => return None,
        }
    }
    None
}

/// Relit le cache des fenêtres, ou `None` s'il est absent ou illisible.
///
/// Le cache existe parce que Claude Code ne renseigne `rate_limits` qu'à partir
/// de la première requête de la session : sans lui, la ligne se réduirait au
/// modèle tant que rien n'a été envoyé.
///
/// Un cache écrit par une version antérieure — plat, sans les deux champs
/// d'ancrage, ou sans la famille de modèle ajoutée le 03/09/2026 — est relu
/// sans erreur : les champs manquants valent `null`, et seul le segment qui en
/// dépend se retire, ou se contente de moins.
///
/// La lecture est réessayée sur un échec passager : voir
/// [`TENTATIVES_LECTURE_CACHE`] pour ce que coûtait l'absence de ce filet.
pub(crate) fn lire_cache() -> Option<Value> {
    let chemin = chemin_cache()?;
    lire_cache_a(&chemin)
}

/// Extrait d'un cache relu l'entrée d'une fenêtre, ou `None` si elle est
/// absente, incomplète ou périmée.
///
/// Une fenêtre dont l'heure de remise à zéro est passée est jetée : le compteur
/// est reparti de zéro entre-temps, et réafficher l'ancien pourcentage serait
/// faux.
pub(crate) fn fenetre_memorisee(cache: Option<&Value>, cle: &str, instant: i64) -> Option<Value> {
    let entree = champ(cache?, cle);
    if entree.is_null() {
        return None;
    }
    if champ(entree, "used_percentage").is_null() || champ(entree, "resets_at").is_null() {
        return None;
    }

    let reset = convertir_instant(champ(entree, "resets_at"))?;
    if reset <= instant {
        return None;
    }

    Some(entree.clone())
}

/// Fenêtre à mémoriser, telle que
/// [`segment_fenetres`](crate::fenetres::segment_fenetres) la remet au cache :
/// les valeurs brutes, avant normalisation.
///
/// `observe_modele` est la famille du modèle sous laquelle l'ancrage a été
/// posé — `"fable"`, `"opus"` — et n'est renseignée que pour les fenêtres qui
/// suivent le modèle ; voir [`FACTEURS_MODELE`](crate::reglages::FACTEURS_MODELE).
pub(crate) struct FenetreAMemoriser {
    pub(crate) cle: &'static str,
    pub(crate) used_percentage: Value,
    pub(crate) resets_at: Value,
    pub(crate) observe_a: Value,
    pub(crate) observe_pourcent: Value,
    pub(crate) observe_modele: Value,
}

/// Entrée du cache, telle qu'elle sera réécrite.
struct EntreeCache {
    cle: &'static str,
    used_percentage: Value,
    resets_at: Value,
    observe_a: Value,
    observe_pourcent: Value,
    observe_modele: Value,
}

/// Sérialise le cache exactement comme le faisait `ConvertTo-Json -Compress`
/// sur une table ordonnée : ordre des clés fixe, champs absents écrits `null`
/// plutôt qu'omis, aucune espace.
///
/// La forme du fichier reste ainsi la même d'une écriture à l'autre, ce que le
/// harnais de non-régression compare octet pour octet. `observe_modele`, ajouté
/// le 03/09/2026, s'écrit en dernier et entre guillemets — `Display` d'une
/// `Value::String` produit la chaîne JSON, échappement compris, comme
/// `ConvertTo-Json` côté oracle.
fn serialiser_cache(entrees: &[EntreeCache]) -> String {
    use std::fmt::Write as _;

    let mut sortie = String::from("{");
    for (index, entree) in entrees.iter().enumerate() {
        if index > 0 {
            sortie.push(',');
        }
        // Écrit en place plutôt que par une chaîne intermédiaire : `write!`
        // sur une `String` ne peut pas échouer, d'où le résultat ignoré.
        let _ = write!(
            sortie,
            "\"{}\":{{\"used_percentage\":{},\"resets_at\":{},\"observe_a\":{},\"observe_pourcent\":{},\"observe_modele\":{}}}",
            entree.cle,
            entree.used_percentage,
            entree.resets_at,
            entree.observe_a,
            entree.observe_pourcent,
            entree.observe_modele
        );
    }
    sortie.push('}');
    sortie
}

/// Mémorise les fenêtres courantes, sans réécrire un contenu identique.
///
/// Les deux fenêtres sont écrites en un seul passage, et `entrees` porte donc
/// aussi celles qui ne viennent que du cache : le payload peut n'en renseigner
/// qu'une, et une écriture partielle perdrait l'autre. Une fenêtre dont le
/// pourcentage n'est pas exploitable est laissée de côté sans empêcher l'autre
/// d'être écrite.
///
/// L'écriture passe par [`ecrire_atomique`], donc par un fichier temporaire
/// renommé : plusieurs sessions Claude Code partagent ce cache, et un
/// déplacement ne laisse jamais lire un fichier à moitié écrit. Toute erreur
/// est absorbée — un cache qu'on n'a pas pu écrire ne doit pas coûter la ligne
/// de statut.
pub(crate) fn ecrire_cache(entrees: &[FenetreAMemoriser]) {
    let Some(chemin) = chemin_cache() else {
        return;
    };

    // Le cache est normalisé — pourcentage entier, secondes Unix, famille en
    // chaîne non blanche ou null — quel que soit le format reçu, pour que la
    // relecture n'ait qu'une seule écriture à interpréter.
    let mut normalise = Vec::new();
    for entree in entrees {
        let Some(pourcent) = convertir_pourcent(&entree.used_percentage) else {
            continue;
        };

        let instant = convertir_instant(&entree.resets_at);
        let observe = convertir_nombre(&entree.observe_a).map(arrondir);
        let modele = match &entree.observe_modele {
            Value::String(s) if !s.trim().is_empty() => Value::String(s.clone()),
            _ => Value::Null,
        };

        normalise.push(EntreeCache {
            cle: entree.cle,
            used_percentage: Value::from(pourcent),
            resets_at: instant.map(Value::from).unwrap_or(Value::Null),
            observe_a: observe
                .filter(|v| *v >= i64::MIN as f64 && *v <= i64::MAX as f64)
                .map(|v| Value::from(v as i64))
                .unwrap_or(Value::Null),
            observe_pourcent: convertir_pourcent(&entree.observe_pourcent)
                .map(Value::from)
                .unwrap_or(Value::Null),
            observe_modele: modele,
        });
    }

    if normalise.is_empty() {
        return;
    }

    let contenu = serialiser_cache(&normalise);

    if let Some(existant) = lire_texte(&chemin) {
        if existant == contenu {
            return;
        }
    }

    ecrire_atomique(&chemin, contenu.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Écrit un contenu dans un fichier jetable et rend son chemin.
    ///
    /// Le dossier porte le PID et l'étiquette du test : les tests s'exécutent en
    /// parallèle, et deux d'entre eux ne doivent pas se disputer un nom. Aucune
    /// dépendance de test n'étant déclarée dans le `Cargo.toml`, le ménage se
    /// fait à la main.
    fn fichier_jetable(etiquette: &str, contenu: &str) -> std::path::PathBuf {
        let dossier = std::env::temp_dir().join(format!(
            "statusline-test-{}-{}",
            std::process::id(),
            etiquette
        ));
        std::fs::create_dir_all(&dossier).expect("dossier jetable");
        let chemin = dossier.join(NOM_CACHE);
        std::fs::write(&chemin, contenu).expect("écriture jetable");
        chemin
    }

    /// Retire le dossier jetable d'un test.
    fn nettoyer(chemin: &std::path::Path) {
        if let Some(dossier) = chemin.parent() {
            let _ = std::fs::remove_dir_all(dossier);
        }
    }

    #[test]
    fn cache_absent_ne_se_reessaie_pas() {
        // « Fichier introuvable » est définitif : c'est l'état du premier
        // lancement d'une session, qui n'a rien à attendre.
        let erreur = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert!(!reessayer(&erreur, 1));
    }

    #[test]
    fn echec_passager_se_reessaie_jusqu_a_la_derniere_tentative() {
        // Un accès refusé est ce que rend Windows pendant le remplacement du
        // cache par une autre instance : passager, donc à réessayer.
        let erreur = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(reessayer(&erreur, 1));
        assert!(reessayer(&erreur, TENTATIVES_LECTURE_CACHE - 1));
        assert!(!reessayer(&erreur, TENTATIVES_LECTURE_CACHE));
    }

    #[test]
    fn cache_lisible_est_relu() {
        let chemin = fichier_jetable(
            "lisible",
            r#"{"five_hour":{"used_percentage":29,"resets_at":1786303800,"observe_a":1786290000,"observe_pourcent":12}}"#,
        );
        let relu = lire_cache_a(&chemin).expect("cache relu");
        assert_eq!(champ(champ(&relu, "five_hour"), "observe_pourcent"), 12);
        nettoyer(&chemin);
    }

    #[test]
    fn cache_introuvable_rend_none() {
        let absent = std::env::temp_dir()
            .join(format!(
                "statusline-test-{}-introuvable",
                std::process::id()
            ))
            .join(NOM_CACHE);
        assert!(lire_cache_a(&absent).is_none());
    }

    #[test]
    fn cache_mal_forme_rend_none_sans_reessayer() {
        // Le contenu est lisible mais invalide : l'écriture passant par un
        // renommage, un JSON tronqué ne peut pas être un état transitoire.
        let chemin = fichier_jetable("tronque", r#"{"five_hour":{"used_per"#);
        assert!(lire_cache_a(&chemin).is_none());
        nettoyer(&chemin);
    }

    #[test]
    fn cache_serialise_avec_ses_nuls() {
        let entrees = vec![EntreeCache {
            cle: "five_hour",
            used_percentage: Value::from(29),
            resets_at: Value::from(1_786_303_800_i64),
            observe_a: Value::Null,
            observe_pourcent: Value::Null,
            observe_modele: Value::Null,
        }];
        assert_eq!(
            serialiser_cache(&entrees),
            "{\"five_hour\":{\"used_percentage\":29,\"resets_at\":1786303800,\
             \"observe_a\":null,\"observe_pourcent\":null,\"observe_modele\":null}}"
        );
    }

    /// La famille s'écrit entre guillemets, en dernier, et une valeur qui n'est
    /// pas une chaîne pleine retombe sur `null` — la forme du fichier ne doit
    /// pas dépendre de ce que le payload a envoyé.
    #[test]
    fn la_famille_est_serialisee_en_chaine_ou_en_nul() {
        let entrees = vec![EntreeCache {
            cle: "seven_day",
            used_percentage: Value::from(41),
            resets_at: Value::from(1_786_633_690_i64),
            observe_a: Value::from(1_786_112_000_i64),
            observe_pourcent: Value::from(18),
            observe_modele: Value::from("fable"),
        }];
        assert_eq!(
            serialiser_cache(&entrees),
            "{\"seven_day\":{\"used_percentage\":41,\"resets_at\":1786633690,\
             \"observe_a\":1786112000,\"observe_pourcent\":18,\"observe_modele\":\"fable\"}}"
        );

        // La normalisation d'`ecrire_cache` est reproduite ici sur ses trois
        // entrées possibles : chaîne pleine, chaîne blanche, autre chose.
        let normaliser = |valeur: Value| match &valeur {
            Value::String(s) if !s.trim().is_empty() => Value::String(s.clone()),
            _ => Value::Null,
        };
        assert_eq!(normaliser(Value::from("opus")), Value::from("opus"));
        assert_eq!(normaliser(Value::from("   ")), Value::Null);
        assert_eq!(normaliser(Value::from(2)), Value::Null);
    }
}
