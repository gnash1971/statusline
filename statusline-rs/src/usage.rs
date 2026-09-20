//! Consommation relevée par `/usage` et persistée dans « ~/.claude.json » —
//! 03/09/2026, au soir.
//!
//! # Ce que le payload ne dit pas, et où Claude Code l'écrit
//!
//! Le contrat d'entrée ne porte que deux fenêtres, `five_hour` et `seven_day`,
//! toutes deux lues dans les en-têtes `anthropic-ratelimit-unified-*` de la
//! dernière réponse API. L'écran `/usage`, lui, affiche une fenêtre de plus
//! pour les modèles qui en ont une — « Current week (Fable) » —, et c'est elle
//! qui décide du passage de Fable aux crédits payants. Sur ce poste, le
//! 03/09/2026 au soir : 14 % pour tous les modèles, 28 % pour Fable.
//!
//! Cette fenêtre vient d'un appel à `/api/oauth/usage`, dont Claude Code
//! persiste la réponse dans `~/.claude.json` sous `cachedUsageUtilization` :
//! un `fetchedAtMs`, l'`accountUuid` du compte, et sous `utilization.limits[]`
//! une entrée par barre de l'écran — `session`, `weekly_all`, et une
//! `weekly_scoped` par modèle, portant `scope.model.display_name`. C'est le
//! même fichier que lit [`crate::abonnement`], et pour la même raison : ce que
//! le payload tait se lit sur le disque, jamais en lançant un processus ni en
//! ouvrant le fichier des jetons.
//!
//! # Ce qu'il faut savoir de sa fraîcheur
//!
//! Relevé dans le bundle de `claude.exe` 2.1.259 : la réponse n'est réécrite
//! sur le disque qu'au plus toutes les cinq minutes, à l'occasion d'un appel
//! — l'ouverture de `/usage`, pour l'essentiel —, et Claude Code lui-même ne
//! relit la valeur persistée que si elle a **moins d'une heure**. Mesuré sur
//! le poste : `fetchedAtMs` n'a pas bougé en dix minutes d'activité soutenue.
//! La ligne applique la même règle, à une nuance près : au-delà de l'heure, la
//! fenêtre reste affichée tant qu'elle n'a pas expiré, mais marquée « ~ » et
//! sans rythme — une valeur ancienne, pas fausse, comme celle qu'un cache
//! restitue au lancement. Voir [`AGE_MAX_USAGE_PERSISTE`].
//!
//! # Ce que vaut une fenêtre pas encore entamée — 04/09/2026
//!
//! Le lendemain d'une remise à zéro hebdomadaire, le relevé du poste portait
//! pour Fable `percent: 0` et **`resets_at: null`** : tant que le modèle n'a
//! rien consommé depuis la remise à zéro, sa fenêtre n'a pas commencé et n'a
//! donc pas d'échéance. `/usage` l'affiche quand même — relevé dans le bundle
//! de `claude.exe` 2.1.259, son composant ne se retire que sur un
//! `utilization` nul, et se borne à omettre la ligne « Resets » quand la date
//! manque. La ligne, elle, tenait cette date nulle pour illisible et rejetait
//! l'entrée : elle retombait sur la fenêtre globale chaque jeudi soir, au
//! moment précis où `/usage` disait « Current week (Fable) 0 % ». Depuis, une
//! date nulle passe telle quelle, et la mise en forme se passe d'échéance —
//! sans rythme ni heure, jamais sans pourcentage.
//!
//! # La dérive entre deux relevés — 05/09/2026
//!
//! Le relevé ne bouge qu'à l'ouverture de `/usage` — revérifié sur la 2.1.261
//! par capture du payload puis scan du bundle : un seul écrivain, sur le chemin
//! de l'appel à `/api/oauth/usage`, que rien ne déclenche périodiquement. Deux
//! heures de travail plus tard, la ligne disait encore « ~17% » quand l'écran
//! allait dire 21. Mais le même relevé porte aussi la fenêtre globale,
//! `weekly_all`, vue au même instant — et celle-là, le payload la donne
//! **fraîche** à chaque réponse, sous `rate_limits.seven_day`. Ce que la
//! globale a pris depuis le relevé, la fenêtre du modèle l'a pris aussi, au
//! rapport des deux budgets ; et ce rapport se lit dans le relevé lui-même :
//! 28 contre 14 le 03/09, 21 contre 11 le 05/09, deux fois dans les deux cas.
//!
//! [`estimer_derive`] avance donc la fenêtre du modèle de l'avance de la
//! globale, multipliée par ce rapport, et rend un pourcentage **estimé** que la
//! ligne affiche marqué « ≈ », sans rythme — ce n'est plus une mesure, et le
//! marqueur le dit. Quatre conditions, faute desquelles le relevé s'affiche tel
//! quel : la même fenêtre globale des deux côtés, à la seconde de sa remise à
//! zéro ; une avance strictement positive — un recul est une autre fenêtre ou
//! une correction, pas une consommation ; et deux pourcentages relevés non
//! nuls, sans quoi le rapport n'existe pas. La limite est assumée : une
//! consommation d'un autre modèle entre deux relevés est comptée comme celle
//! du modèle courant. Le cache, lui, mémorise toujours le relevé, jamais
//! l'estimation.

use serde_json::Value;

use crate::conversions::{arrondir, champ, convertir_instant, convertir_pourcent};
use crate::reglages::{
    AGE_MAX_USAGE_PERSISTE, CLE_USAGE_PERSISTE, GENRE_FENETRE_GLOBALE, GENRE_FENETRE_MODELE,
};

/// Fenêtre hebdomadaire propre au modèle courant, telle que `/usage` l'a
/// relevée.
///
/// Les deux valeurs du payload sont gardées **brutes** — `percent` et
/// `resets_at` tels qu'écrits — pour passer par les mêmes conversions que
/// celles d'une fenêtre du payload de session ; `releve_a` est l'instant du
/// relevé en secondes Unix, et c'est lui qui date la mesure.
pub(crate) struct FenetreModele {
    pub(crate) used_percentage: Value,
    pub(crate) resets_at: Value,
    pub(crate) releve_a: i64,
    /// Fenêtre de tous les modèles, vue par le même relevé : le point de
    /// référence de la dérive. `None` quand le relevé n'en porte pas de
    /// lisible, et la fenêtre du modèle s'affiche alors telle que relevée.
    pub(crate) reference_globale: Option<ReferenceGlobale>,
}

/// Fenêtre hebdomadaire de tous les modèles, telle que le relevé l'a vue —
/// déjà converties, parce qu'elles ne servent qu'à un calcul : voir
/// [`estimer_derive`].
#[derive(Clone, Copy)]
pub(crate) struct ReferenceGlobale {
    pub(crate) pourcent: i64,
    /// Remise à zéro en secondes Unix : c'est elle qui permet de reconnaître
    /// la même fenêtre dans le payload.
    pub(crate) reset: i64,
}

impl FenetreModele {
    /// Indique si le relevé a dépassé l'âge au-delà duquel la ligne le marque
    /// comme ancien.
    pub(crate) fn ancienne(&self, instant: i64) -> bool {
        instant - self.releve_a > AGE_MAX_USAGE_PERSISTE
    }
}

/// Rend la fenêtre hebdomadaire propre à la famille du modèle, ou `None`.
///
/// `None` couvre tout ce qui ne permet pas d'afficher une valeur honnête :
/// configuration absente ou muette, relevé d'un autre compte que celui
/// connecté — Claude Code fait la même vérification —, relevé daté d'un
/// instant qui n'est pas encore venu, entrée dont le pourcentage ne se lit
/// pas, remise à zéro illisible ou déjà passée, et bien sûr famille inconnue
/// ou sans fenêtre propre. La première entrée `weekly_scoped` dont le modèle
/// porte le nom de la famille l'emporte, la casse ignorée.
///
/// Une remise à zéro **nulle** n'est pas une remise à zéro illisible : c'est
/// une fenêtre pas encore entamée, qui passe avec son pourcentage et sans
/// échéance — voir le module.
pub(crate) fn fenetre_modele(
    config: Option<&Value>,
    famille: Option<&str>,
    instant: i64,
) -> Option<FenetreModele> {
    let config = config?;
    let famille = famille?;
    let usage = champ(config, CLE_USAGE_PERSISTE);
    if usage.is_null() {
        return None;
    }

    if !meme_compte(config, usage) {
        return None;
    }

    let releve_a = convertir_instant(champ(usage, "fetchedAtMs"))?;
    if releve_a > instant {
        return None;
    }

    let Value::Array(limites) = champ(champ(usage, "utilization"), "limits") else {
        return None;
    };

    let reference_globale = reference_globale(limites);

    limites
        .iter()
        .filter(|limite| est_fenetre_de(limite, famille))
        .find_map(|limite| {
            let used_percentage = champ(limite, "percent");
            convertir_pourcent(used_percentage)?;

            // Nulle, la remise à zéro est celle d'une fenêtre pas encore
            // entamée et passe telle quelle. Toute autre valeur doit se lire,
            // et désigner un instant à venir.
            let resets_at = champ(limite, "resets_at");
            if !resets_at.is_null() {
                let reset = convertir_instant(resets_at)?;
                if reset <= instant {
                    return None;
                }
            }

            Some(FenetreModele {
                used_percentage: used_percentage.clone(),
                resets_at: resets_at.clone(),
                releve_a,
                reference_globale,
            })
        })
}

/// Rend la fenêtre hebdomadaire de tous les modèles portée par le relevé, ou
/// `None` si aucune entrée `weekly_all` n'y est lisible — pourcentage et
/// remise à zéro tous deux exigés, la seconde servant à reconnaître la même
/// fenêtre dans le payload. La première entrée lisible l'emporte.
fn reference_globale(limites: &[Value]) -> Option<ReferenceGlobale> {
    limites
        .iter()
        .filter(|limite| {
            matches!(champ(limite, "kind"), Value::String(genre) if genre == GENRE_FENETRE_GLOBALE)
        })
        .find_map(|limite| {
            let pourcent = convertir_pourcent(champ(limite, "percent"))?;
            let reset = convertir_instant(champ(limite, "resets_at"))?;
            Some(ReferenceGlobale { pourcent, reset })
        })
}

/// Estime où en est la fenêtre du modèle depuis son relevé, d'après l'avance
/// prise entre-temps par la fenêtre globale du payload — ou `None` quand rien
/// d'honnête ne peut être estimé, et le relevé s'affiche alors tel quel.
///
/// `globale` est `rate_limits.seven_day` du payload, fraîche à chaque réponse.
/// Ce qu'elle a pris depuis le relevé, la fenêtre du modèle l'a pris aussi, au
/// rapport des deux budgets, et ce rapport se lit dans le relevé : le
/// pourcentage du modèle sur celui de la globale, au même instant. Le résultat
/// est arrondi comme tout pourcentage de la ligne, et plafonné à cent.
///
/// `None` couvre : un relevé sans référence globale lisible ; une globale du
/// payload illisible, ou dont la remise à zéro n'est pas celle du relevé — ce
/// n'est plus la même fenêtre ; une avance nulle ou négative — un recul est une
/// correction côté serveur, pas une consommation ; et un rapport qui n'existe
/// pas, l'un des deux pourcentages relevés étant nul — c'est l'état du
/// lendemain d'une remise à zéro, où la fenêtre du modèle s'affiche à 0 % tant
/// que `/usage` n'a pas été rouvert.
pub(crate) fn estimer_derive(releve: &FenetreModele, globale: &Value) -> Option<i64> {
    let reference = releve.reference_globale?;
    if reference.pourcent <= 0 {
        return None;
    }
    let releve_pourcent = convertir_pourcent(&releve.used_percentage)?;
    if releve_pourcent <= 0 {
        return None;
    }

    let courant = convertir_pourcent(champ(globale, "used_percentage"))?;
    let reset = convertir_instant(champ(globale, "resets_at"))?;
    if reset != reference.reset {
        return None;
    }

    let avance = courant - reference.pourcent;
    if avance <= 0 {
        return None;
    }

    // Même ordre d'opérations que l'oracle, pour tomber sur le même double
    // avant l'arrondi : le rapport d'abord, puis son produit par l'avance.
    let rapport = releve_pourcent as f64 / reference.pourcent as f64;
    let estime = arrondir(releve_pourcent as f64 + rapport * avance as f64);
    Some((estime as i64).clamp(0, 100))
}

/// Indique si le relevé persisté appartient au compte connecté.
///
/// Les deux identifiants ne sont comparés que s'ils existent tous deux : un
/// relevé sans compte, ou une configuration sans compte, n'est pas un
/// désaccord mais une inconnue, et la fenêtre s'affiche.
fn meme_compte(config: &Value, usage: &Value) -> bool {
    let connecte = champ(champ(config, "oauthAccount"), "accountUuid");
    let releve = champ(usage, "accountUuid");
    match (connecte, releve) {
        (Value::String(a), Value::String(b)) => a == b,
        _ => true,
    }
}

/// Indique si une entrée de `limits[]` est la fenêtre hebdomadaire propre à la
/// famille donnée : le bon genre, et un modèle dont le nom porte celui de la
/// famille.
fn est_fenetre_de(limite: &Value, famille: &str) -> bool {
    let Value::String(genre) = champ(limite, "kind") else {
        return false;
    };
    if genre != GENRE_FENETRE_MODELE {
        return false;
    }

    let modele = champ(champ(limite, "scope"), "model");
    match champ(modele, "display_name") {
        Value::String(nom) => nom.to_lowercase().contains(famille),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const MAINTENANT: i64 = 1_788_463_900;

    /// Configuration telle que Claude Code l'écrit, à quelques champs près.
    fn config(fetched_ms: i64, limites: Value) -> Value {
        json!({
            "oauthAccount": { "accountUuid": "compte-a", "organizationType": "claude_max" },
            "cachedUsageUtilization": {
                "fetchedAtMs": fetched_ms,
                "accountUuid": "compte-a",
                "utilization": { "limits": limites }
            }
        })
    }

    fn limites_fable(pourcent: Value, reset: &str) -> Value {
        limites_fable_brutes(pourcent, json!(reset))
    }

    /// Même relevé, la remise à zéro de la fenêtre Fable donnée brute — ce qui
    /// permet d'y mettre `null`, comme l'API le fait pour une fenêtre pas
    /// encore entamée.
    fn limites_fable_brutes(pourcent: Value, reset: Value) -> Value {
        json!([
            { "kind": "session", "group": "session", "percent": 36, "resets_at": "2026-09-04T00:30:00+02:00", "scope": null },
            { "kind": "weekly_all", "group": "weekly", "percent": 14, "resets_at": RESET_FUTUR, "scope": null },
            { "kind": "weekly_scoped", "group": "weekly", "percent": pourcent, "resets_at": reset,
              "scope": { "model": { "id": null, "display_name": "Fable" }, "surface": null } }
        ])
    }

    const RESET_FUTUR: &str = "2026-09-07T23:00:00.185454+02:00";
    const RESET_PASSE: &str = "2026-09-01T23:00:00+02:00";

    #[test]
    fn la_fenetre_du_modele_est_lue_dans_limits() {
        let cfg = config(
            MAINTENANT * 1000 - 60_000,
            limites_fable(json!(28), RESET_FUTUR),
        );
        let fenetre = fenetre_modele(Some(&cfg), Some("fable"), MAINTENANT).expect("fenêtre Fable");

        assert_eq!(fenetre.used_percentage, json!(28));
        assert_eq!(fenetre.resets_at, json!(RESET_FUTUR));
        // Le relevé est daté à la seconde, depuis des millisecondes.
        assert_eq!(fenetre.releve_a, MAINTENANT - 60);
        assert!(!fenetre.ancienne(MAINTENANT));
        assert!(fenetre.ancienne(MAINTENANT + AGE_MAX_USAGE_PERSISTE + 1));
    }

    #[test]
    fn seule_la_famille_du_modele_courant_trouve_sa_fenetre() {
        let cfg = config(MAINTENANT * 1000, limites_fable(json!(28), RESET_FUTUR));
        assert!(fenetre_modele(Some(&cfg), Some("opus"), MAINTENANT).is_none());
        assert!(fenetre_modele(Some(&cfg), None, MAINTENANT).is_none());
        assert!(fenetre_modele(None, Some("fable"), MAINTENANT).is_none());
        // Le nom du modèle se compare sans la casse.
        assert!(fenetre_modele(Some(&cfg), Some("fable"), MAINTENANT).is_some());
    }

    #[test]
    fn ce_qui_retire_la_fenetre() {
        // Fenêtre déjà remise à zéro : le compteur est reparti, la valeur est
        // fausse.
        let expiree = config(MAINTENANT * 1000, limites_fable(json!(28), RESET_PASSE));
        assert!(fenetre_modele(Some(&expiree), Some("fable"), MAINTENANT).is_none());

        // Pourcentage illisible.
        let illisible = config(
            MAINTENANT * 1000,
            limites_fable(json!("beaucoup"), RESET_FUTUR),
        );
        assert!(fenetre_modele(Some(&illisible), Some("fable"), MAINTENANT).is_none());

        // Relevé daté du futur : l'horloge du poste a reculé.
        let futur = config(
            (MAINTENANT + 3_600) * 1000,
            limites_fable(json!(28), RESET_FUTUR),
        );
        assert!(fenetre_modele(Some(&futur), Some("fable"), MAINTENANT).is_none());

        // `limits` qui n'est pas un tableau, ou relevé absent.
        let mut sans_tableau = config(MAINTENANT * 1000, json!(28));
        assert!(fenetre_modele(Some(&sans_tableau), Some("fable"), MAINTENANT).is_none());
        sans_tableau["cachedUsageUtilization"] = Value::Null;
        assert!(fenetre_modele(Some(&sans_tableau), Some("fable"), MAINTENANT).is_none());

        // Relevé d'un autre compte que celui connecté.
        let mut autre_compte = config(MAINTENANT * 1000, limites_fable(json!(28), RESET_FUTUR));
        autre_compte["cachedUsageUtilization"]["accountUuid"] = json!("compte-b");
        assert!(fenetre_modele(Some(&autre_compte), Some("fable"), MAINTENANT).is_none());

        // Sans identifiant de l'un ou de l'autre côté, pas de désaccord.
        autre_compte["cachedUsageUtilization"]["accountUuid"] = Value::Null;
        assert!(fenetre_modele(Some(&autre_compte), Some("fable"), MAINTENANT).is_some());
    }

    /// Le relevé du 04/09/2026, lendemain de remise à zéro : 0 % et pas
    /// d'échéance. La fenêtre passe, brute, et c'est la mise en forme qui se
    /// passera de l'heure et du rythme.
    #[test]
    fn une_fenetre_pas_encore_entamee_passe_sans_echeance() {
        let cfg = config(
            MAINTENANT * 1000 - 60_000,
            limites_fable_brutes(json!(0), Value::Null),
        );
        let fenetre = fenetre_modele(Some(&cfg), Some("fable"), MAINTENANT).expect("fenêtre Fable");

        assert_eq!(fenetre.used_percentage, json!(0));
        assert_eq!(fenetre.resets_at, Value::Null);
        assert_eq!(fenetre.releve_a, MAINTENANT - 60);

        // Une remise à zéro qui n'est ni nulle ni lisible reste un rejet : la
        // tolérance vaut pour l'absence, pas pour le bruit.
        let illisible = config(
            MAINTENANT * 1000,
            limites_fable_brutes(json!(28), json!("jamais")),
        );
        assert!(fenetre_modele(Some(&illisible), Some("fable"), MAINTENANT).is_none());
    }

    #[test]
    fn une_entree_sans_modele_ou_d_un_autre_genre_est_ignoree() {
        let cfg = config(
            MAINTENANT * 1000,
            json!([
                { "kind": "weekly_all", "percent": 90, "resets_at": RESET_FUTUR, "scope": { "model": { "display_name": "Fable" } } },
                { "kind": "weekly_scoped", "percent": 50, "resets_at": RESET_FUTUR, "scope": null },
                { "kind": "weekly_scoped", "percent": 28, "resets_at": RESET_FUTUR, "scope": { "model": { "display_name": "Fable 5.1" } } }
            ]),
        );
        let fenetre = fenetre_modele(Some(&cfg), Some("fable"), MAINTENANT).expect("fenêtre Fable");
        assert_eq!(fenetre.used_percentage, json!(28));
    }

    /// Le relevé porte aussi la fenêtre globale, vue au même instant : c'est
    /// le point de référence de la dérive. Sans entrée globale lisible, la
    /// fenêtre du modèle passe quand même, sans référence.
    #[test]
    fn la_reference_globale_est_lue_dans_le_meme_releve() {
        let cfg = config(MAINTENANT * 1000, limites_fable(json!(28), RESET_FUTUR));
        let fenetre = fenetre_modele(Some(&cfg), Some("fable"), MAINTENANT).expect("fenêtre Fable");
        let reference = fenetre.reference_globale.expect("référence globale");
        assert_eq!(reference.pourcent, 14);
        assert_eq!(
            reference.reset,
            convertir_instant(&json!(RESET_FUTUR)).expect("instant")
        );

        let scoped = json!({ "kind": "weekly_scoped", "percent": 28, "resets_at": RESET_FUTUR,
            "scope": { "model": { "display_name": "Fable" } } });

        let sans_globale = config(MAINTENANT * 1000, json!([scoped]));
        let fenetre =
            fenetre_modele(Some(&sans_globale), Some("fable"), MAINTENANT).expect("fenêtre Fable");
        assert!(fenetre.reference_globale.is_none());

        // Une globale sans échéance ne permet pas de reconnaître la fenêtre
        // du payload : pas de référence.
        let sans_echeance = config(
            MAINTENANT * 1000,
            json!([{ "kind": "weekly_all", "percent": 14, "resets_at": null, "scope": null }, scoped]),
        );
        let fenetre =
            fenetre_modele(Some(&sans_echeance), Some("fable"), MAINTENANT).expect("fenêtre Fable");
        assert!(fenetre.reference_globale.is_none());
    }

    /// Relevé fabriqué de toutes pièces : la fenêtre Fable, et la globale
    /// telle que le même relevé l'a vue.
    fn releve(fable: Value, globale: Option<(i64, i64)>) -> FenetreModele {
        FenetreModele {
            used_percentage: fable,
            resets_at: json!(RESET_FUTUR),
            releve_a: MAINTENANT - 3_600,
            reference_globale: globale
                .map(|(pourcent, reset)| ReferenceGlobale { pourcent, reset }),
        }
    }

    /// Fenêtre globale du payload, telle que Claude Code l'envoie : secondes
    /// Unix pour la remise à zéro.
    fn globale_payload(pourcent: i64, reset: i64) -> Value {
        json!({ "used_percentage": pourcent, "resets_at": reset })
    }

    /// Le relevé du poste : 28 % pour Fable contre 14 % pour tous. Deux
    /// points pris par la globale depuis, au rapport de deux : 32.
    #[test]
    fn la_derive_avance_la_fenetre_du_modele_au_rapport_des_budgets() {
        let reset = convertir_instant(&json!(RESET_FUTUR)).expect("instant");
        let r = releve(json!(28), Some((14, reset)));
        assert_eq!(estimer_derive(&r, &globale_payload(16, reset)), Some(32));

        // Le relevé du 05/09/2026 : 21 contre 11, deux points d'avance —
        // 21 + 2 × 21/11 = 24,8, arrondi à 25.
        let r = releve(json!(21), Some((11, reset)));
        assert_eq!(estimer_derive(&r, &globale_payload(13, reset)), Some(25));

        // La remise à zéro du payload peut s'écrire en ISO : même seconde.
        let r = releve(json!(28), Some((14, reset)));
        assert_eq!(
            estimer_derive(
                &r,
                &json!({ "used_percentage": 16, "resets_at": RESET_FUTUR })
            ),
            Some(32)
        );
    }

    /// Arrondi du banquier, comme partout sur la ligne : 27,5 va à 28, 38,5
    /// à 38. Et le plafond est cent — au-delà, l'estimation dit seulement que
    /// la fenêtre est pleine.
    #[test]
    fn la_derive_s_arrondit_au_pair_et_se_plafonne_a_cent() {
        let reset = 1_788_800_000;
        assert_eq!(
            estimer_derive(
                &releve(json!(25), Some((10, reset))),
                &globale_payload(11, reset)
            ),
            Some(28)
        );
        assert_eq!(
            estimer_derive(
                &releve(json!(35), Some((10, reset))),
                &globale_payload(11, reset)
            ),
            Some(38)
        );
        assert_eq!(
            estimer_derive(
                &releve(json!(28), Some((14, reset))),
                &globale_payload(60, reset)
            ),
            Some(100)
        );
    }

    /// Tout ce qui retire la dérive : le relevé s'affiche alors tel quel.
    #[test]
    fn ce_qui_retire_la_derive() {
        let reset = 1_788_800_000;
        let r = releve(json!(28), Some((14, reset)));

        // Aucune avance, ou un recul : rien à estimer.
        assert_eq!(estimer_derive(&r, &globale_payload(14, reset)), None);
        assert_eq!(estimer_derive(&r, &globale_payload(10, reset)), None);

        // Une autre fenêtre globale — remise à zéro différente —, ou une
        // globale du payload illisible ou absente.
        assert_eq!(
            estimer_derive(&r, &globale_payload(16, reset + 3_600)),
            None
        );
        assert_eq!(
            estimer_derive(&r, &json!({ "used_percentage": 16, "resets_at": null })),
            None
        );
        assert_eq!(
            estimer_derive(
                &r,
                &json!({ "used_percentage": "beaucoup", "resets_at": reset })
            ),
            None
        );
        assert_eq!(estimer_derive(&r, &Value::Null), None);

        // Sans référence globale dans le relevé.
        assert_eq!(
            estimer_derive(&releve(json!(28), None), &globale_payload(16, reset)),
            None
        );

        // Un rapport qui n'existe pas : globale ou fenêtre du modèle à zéro —
        // l'état du lendemain d'une remise à zéro.
        assert_eq!(
            estimer_derive(
                &releve(json!(28), Some((0, reset))),
                &globale_payload(16, reset)
            ),
            None
        );
        assert_eq!(
            estimer_derive(
                &releve(json!(0), Some((2, reset))),
                &globale_payload(10, reset)
            ),
            None
        );

        // Pourcentage du relevé illisible.
        assert_eq!(
            estimer_derive(
                &releve(json!("beaucoup"), Some((14, reset))),
                &globale_payload(16, reset)
            ),
            None
        );
    }
}
