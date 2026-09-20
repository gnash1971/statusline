//! Famille du modèle et facteur de consommation — 03/09/2026.
//!
//! La fenêtre hebdomadaire suit le modèle qui tourne : un modèle deux fois plus
//! cher par jeton vide le même budget deux fois plus vite, et l'alerte doit
//! arriver d'autant plus tôt. Ce module ne fait que lire la famille du modèle
//! et en tirer un descripteur de fenêtre ajusté ; ce que le facteur change au
//! rythme vit dans [`crate::fenetres`], et la table des facteurs dans
//! [`FACTEURS_MODELE`].

use serde_json::Value;

use crate::conversions::{arrondir, champ};
use crate::reglages::{Fenetre, FACTEURS_MODELE};

/// Rend la famille du modèle de la session, ou `None` si elle est inconnue.
///
/// Lue dans `model.id` d'abord — `claude-fable-5-1` —, puis dans
/// `model.display_name` en repli, pour un payload qui n'aurait que le nom. La
/// famille est le premier mot de [`FACTEURS_MODELE`] que le nom contient, la
/// casse ignorée ; un modèle hors table rend `None`, et la ligne se comporte
/// alors comme avant cette évolution.
pub(crate) fn famille_modele(donnees: &Value) -> Option<&'static str> {
    let modele = champ(donnees, "model");
    for cle in ["id", "display_name"] {
        if let Value::String(nom) = champ(modele, cle) {
            if let Some(famille) = famille_du_nom(nom) {
                return Some(famille);
            }
        }
    }
    None
}

/// Cœur de [`famille_modele`], sur un nom déjà extrait.
fn famille_du_nom(nom: &str) -> Option<&'static str> {
    let minuscule = nom.to_lowercase();
    FACTEURS_MODELE
        .iter()
        .map(|(famille, _)| *famille)
        .find(|famille| minuscule.contains(famille))
}

/// Facteur de consommation d'une famille, rapporté à Opus : 1 pour une famille
/// inconnue ou absente.
pub(crate) fn facteur_famille(famille: Option<&str>) -> f64 {
    famille
        .and_then(|f| FACTEURS_MODELE.iter().find(|(nom, _)| *nom == f))
        .map(|(_, facteur)| *facteur)
        .unwrap_or(1.0)
}

/// Abaisse un seuil selon le facteur, sans jamais le relever.
///
/// Un seuil laisse une **réserve** — les points qui le séparent du plafond —
/// et c'est elle qui donne le temps de lever le pied. Sous un modèle qui
/// consomme `facteur` fois plus vite, la même réserve dure `facteur` fois
/// moins longtemps : pour garder le même délai, elle doit être `facteur` fois
/// plus large, et le seuil descend d'autant. 75 % devient 50 % sous un facteur
/// 2, 85 % devient 70 %.
///
/// Un facteur inférieur à 1 ne relève rien. Le compteur est partagé entre les
/// modèles, et une session Sonnet ne rend pas moins cher ce que Fable a déjà
/// consommé la même semaine ; abaisser le seuil est une précaution sur ce qui
/// vient, le relever effacerait une alerte sur ce qui est déjà là.
pub(crate) fn adapter_seuil(seuil: i64, facteur: f64) -> i64 {
    if facteur <= 1.0 {
        return seuil;
    }
    let reserve = arrondir((100 - seuil) as f64 * facteur) as i64;
    (100 - reserve).max(0)
}

/// Rend le descripteur d'une fenêtre tel qu'il s'applique sous ce facteur.
///
/// Seules les fenêtres qui déclarent [`Fenetre::seuils_selon_modele`] bougent ;
/// les autres reviennent telles quelles, ce qui garde à la fenêtre de 5 heures
/// et à la fenêtre propre au modèle les seuils écrits dans
/// [`crate::reglages::FENETRES`] et [`crate::reglages::FENETRE_MODELE`].
pub(crate) fn descripteur_effectif(descripteur: &Fenetre, facteur: f64) -> Fenetre {
    if !descripteur.seuils_selon_modele {
        return *descripteur;
    }
    Fenetre {
        seuil: adapter_seuil(descripteur.seuil, facteur),
        seuil_critique: adapter_seuil(descripteur.seuil_critique, facteur),
        ..*descripteur
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::reglages::{FENETRES, FENETRE_MODELE, SEUIL_CRITIQUE_FENETRE_7J, SEUIL_FENETRE_7J};

    #[test]
    fn la_famille_se_lit_dans_l_identifiant_puis_dans_le_nom() {
        assert_eq!(
            famille_modele(
                &json!({ "model": { "id": "claude-fable-5-1", "display_name": "Fable 5.1" } })
            ),
            Some("fable")
        );
        assert_eq!(
            famille_modele(&json!({ "model": { "id": "claude-opus-5" } })),
            Some("opus")
        );
        assert_eq!(
            famille_modele(&json!({ "model": { "id": "claude-mythos-5-1" } })),
            Some("mythos")
        );
        // Sans identifiant, le nom affiché suffit, quelle que soit sa casse.
        assert_eq!(
            famille_modele(&json!({ "model": { "display_name": "SONNET 5" } })),
            Some("sonnet")
        );
        // Un identifiant qui n'est pas une chaîne n'empêche pas le repli.
        assert_eq!(
            famille_modele(&json!({ "model": { "id": 5, "display_name": "Haiku 4.5" } })),
            Some("haiku")
        );
        // Hors table, ou sans modèle : inconnue, et rien ne change.
        assert_eq!(
            famille_modele(
                &json!({ "model": { "id": "claude-ultra-6", "display_name": "Ultra 6" } })
            ),
            None
        );
        assert_eq!(famille_modele(&json!({})), None);
        assert_eq!(famille_modele(&json!([1, 2])), None);
    }

    #[test]
    fn le_facteur_vaut_un_hors_table() {
        assert_eq!(facteur_famille(Some("fable")), 2.0);
        assert_eq!(facteur_famille(Some("opus")), 1.0);
        assert_eq!(facteur_famille(Some("ultra")), 1.0);
        assert_eq!(facteur_famille(None), 1.0);
    }

    #[test]
    fn le_seuil_descend_avec_le_facteur_et_ne_remonte_jamais() {
        // La réserve double : 25 points deviennent 50, 15 deviennent 30.
        assert_eq!(adapter_seuil(75, 2.0), 50);
        assert_eq!(adapter_seuil(85, 2.0), 70);
        // Facteur neutre, ou inférieur à un : le seuil reste celui d'Opus.
        assert_eq!(adapter_seuil(75, 1.0), 75);
        assert_eq!(adapter_seuil(75, 0.4), 75);
        // Un facteur démesuré ne produit pas un seuil négatif.
        assert_eq!(adapter_seuil(75, 10.0), 0);
        // Une réserve fractionnaire s'arrondit comme partout ailleurs.
        assert_eq!(adapter_seuil(85, 1.5), 78);
    }

    #[test]
    fn seule_la_fenetre_hebdomadaire_suit_le_modele() {
        let cinq_heures = &FENETRES[0];
        let sept_jours = &FENETRES[1];
        assert!(!cinq_heures.seuils_selon_modele);
        assert!(!cinq_heures.ancrage_selon_modele);
        assert!(sept_jours.seuils_selon_modele);
        assert!(sept_jours.ancrage_selon_modele);
        // La fenêtre propre au modèle ne suit que par l'ancrage : son budget
        // est déjà celui du modèle, et ses seuils restent ceux d'Opus. Lue par
        // référence comme les deux autres : une assertion écrite directement
        // sur la constante serait pliée à la compilation, et clippy la refuse
        // (`assertions_on_constants`).
        let fenetre_modele = &FENETRE_MODELE;
        assert!(!fenetre_modele.seuils_selon_modele);
        assert!(fenetre_modele.ancrage_selon_modele);

        let facteur = facteur_famille(Some("fable"));

        let inchangee = descripteur_effectif(cinq_heures, facteur);
        assert_eq!(inchangee.seuil, cinq_heures.seuil);
        assert_eq!(inchangee.seuil_critique, cinq_heures.seuil_critique);

        let modele = descripteur_effectif(&FENETRE_MODELE, facteur);
        assert_eq!(modele.seuil, SEUIL_FENETRE_7J);
        assert_eq!(modele.seuil_critique, SEUIL_CRITIQUE_FENETRE_7J);

        let adaptee = descripteur_effectif(sept_jours, facteur);
        assert_eq!(adaptee.seuil, adapter_seuil(SEUIL_FENETRE_7J, facteur));
        assert_eq!(
            adaptee.seuil_critique,
            adapter_seuil(SEUIL_CRITIQUE_FENETRE_7J, facteur)
        );
        // L'ordre des deux paliers survit à l'adaptation : le critique reste
        // au-dessus de l'alerte, sans quoi l'ambre deviendrait inatteignable.
        assert!(adaptee.seuil < adaptee.seuil_critique);
        // Le reste du descripteur voyage tel quel.
        assert_eq!(adaptee.cle, sept_jours.cle);
        assert_eq!(adaptee.libelle, sept_jours.libelle);
        assert_eq!(adaptee.reset_toujours, sept_jours.reset_toujours);
    }

    /// Chaque famille de la table qui coûte plus qu'Opus doit abaisser les
    /// deux paliers en gardant leur ordre.
    #[test]
    fn toutes_les_familles_gardent_deux_paliers_ordonnes() {
        for (famille, facteur) in FACTEURS_MODELE {
            let adaptee = descripteur_effectif(&FENETRES[1], facteur);
            assert!(
                adaptee.seuil < adaptee.seuil_critique,
                "la famille {famille} confond ses deux paliers"
            );
            if facteur > 1.0 {
                assert!(
                    adaptee.seuil < SEUIL_FENETRE_7J,
                    "{famille} n'abaisse pas l'alerte"
                );
            } else {
                assert_eq!(
                    adaptee.seuil, SEUIL_FENETRE_7J,
                    "{famille} déplace l'alerte"
                );
            }
        }
    }
}
