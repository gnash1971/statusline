//! Abonnement du compte connecté — troisième lecture disque de la section 5.
//!
//! # Pourquoi le disque, et pas le payload
//!
//! Le contrat d'entrée ne porte **rien** de l'abonnement. Ce n'est pas une
//! supposition : le payload de Claude Code 2.1.246, relevé par [`crate::capture`]
//! le 26/08/2026, compte trente-quatre champs — session, modèle, effort,
//! emplacement, coût, contexte, mode rapide, réflexion, fenêtres de limitation —
//! et aucun ne dit sur quel abonnement la session tourne.
//!
//! L'information vit donc là où Claude Code l'écrit lui-même : `~/.claude.json`,
//! sous `oauthAccount.organizationType`. C'est le même mouvement que la version
//! du binaire ([`crate::binaire`]) et la branche Git ([`crate::depot`]) — ce que
//! le payload ne dit pas se lit sur le disque, jamais en lançant un processus.
//!
//! # Le fichier lu, et celui qui ne l'est pas
//!
//! Deux fichiers portent la réponse, et le choix entre eux n'est pas indifférent :
//!
//! | Fichier | Champ | Valeur | Taille | Secrets |
//! |---|---|---|---|---|
//! | `~/.claude.json` | `oauthAccount.organizationType` | `claude_pro` | 50 Ko | aucun |
//! | `~/.claude/.credentials.json` | `claudeAiOauth.subscriptionType` | `pro` | 500 o | **jeton OAuth** |
//!
//! Le second est vingt fois plus court, et c'est le premier qui est lu. Une ligne
//! de statut n'a aucune raison d'ouvrir le fichier qui porte le jeton d'accès et
//! son jeton de rafraîchissement : un programme qui ne l'ouvre jamais ne peut pas
//! le divulguer, quelle que soit la suite de son histoire. Le workspace applique
//! déjà cette règle à sa sauvegarde, qui **refuse** de traiter `.credentials.json`
//! — voir `user-config/README.md`, « Ce qui n'est jamais copié ici ».
//!
//! Le prix est une lecture de 50 Ko par rafraîchissement, **relue et analysée
//! en entier**. Mesuré le 26/08/2026 par le journal du crate, 60 tirs de chaque
//! côté : la médiane interne passe de 1 757 µs à 3 023 µs, soit **+1,3 ms**. Ce
//! n'est pas rien — le programme y double presque son temps propre — mais c'est
//! peu au regard des ~12,6 ms de démarrage de processus que Claude Code paie de
//! toute façon à chaque rafraîchissement : le lancement complet passe de 14,4 à
//! 14,7 ms.
//!
//! Découper le fichier autour de `oauthAccount` pour n'analyser qu'un kilo-octet
//! ramènerait ce surcoût à presque rien. C'est écarté tant que le budget tient :
//! un analyseur d'accolades maison — qu'il faudrait porter à l'identique dans
//! l'oracle — coûterait plus en surface de bug qu'il ne rapporte en
//! microsecondes.
//!
//! # Les valeurs, relevées et non supposées
//!
//! La table de correspondance vient du bundle JavaScript de `claude.exe`, où
//! Claude Code convertit lui-même le type d'organisation en type d'abonnement :
//!
//! ```text
//! new Map([["claude_max","max"],["claude_pro","pro"],
//!          ["claude_enterprise","enterprise"],["claude_team","team"]])
//! ```
//!
//! Ces quatre-là sont donc l'ensemble complet à la version 2.1.246, et
//! [`ABONNEMENTS`](crate::reglages::ABONNEMENTS) les reprend un pour un. Le
//! palier d'un abonnement Max se lit à part, dans `organizationRateLimitTier` :
//! le même bundle y teste `default_claude_max_5x` et `default_claude_max_20x`,
//! d'où [`PALIERS_MAX`](crate::reglages::PALIERS_MAX).
//!
//! Une valeur hors table n'est pas perdue pour autant : [`embellir`] la rend
//! lisible telle quelle. Un abonnement inventé après cette version s'affichera
//! donc approximativement plutôt que pas du tout — ce qui vaut mieux, l'ensemble
//! ci-dessus n'étant complet que jusqu'à la prochaine mise à jour.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::conversions::champ;
use crate::disque::lire_texte;
use crate::reglages::{ABONNEMENTS, CHEMIN_CONFIG_RELATIF, PALIERS_MAX};

/// Rend le chemin de la configuration Claude Code, ou `None` s'il est
/// introuvable.
///
/// `CLAUDE_STATUSLINE_CONFIG` l'emporte, et **sans repli** : c'est la convention
/// de [`CLAUDE_STATUSLINE_BINAIRE`](crate::binaire), et pour la même raison — une
/// désignation explicite qui ne résout pas doit se voir, pas se faire remplacer
/// en silence par l'emplacement habituel. Le harnais de non-régression s'en sert
/// pour couvrir les replis sans rien supposer du compte réel du poste.
fn trouver_config() -> Option<PathBuf> {
    if let Ok(explicite) = std::env::var("CLAUDE_STATUSLINE_CONFIG") {
        if !explicite.is_empty() {
            let chemin = PathBuf::from(explicite);
            return if chemin.is_file() { Some(chemin) } else { None };
        }
    }

    let profil = std::env::var("USERPROFILE")
        .ok()
        .filter(|v| !v.is_empty())?;
    let natif = Path::new(&profil).join(CHEMIN_CONFIG_RELATIF);
    if natif.is_file() {
        Some(natif)
    } else {
        None
    }
}

/// Rend lisible un identifiant en `snake_case` dont la table ne sait rien.
///
/// `claude_pro` devient « Pro ». C'est exactement ce que la table produit pour
/// les valeurs connues, et c'est voulu : le jour où Claude Code ajoute un type
/// d'abonnement, la ligne l'affiche correctement sans qu'on ait rien à
/// recompiler. La table reste utile pour les cas où cette mécanique ne suffirait
/// pas — un palier à joindre, une casse à respecter.
///
/// **Le « claude » de tête est élagué depuis le 26/08/2026**, comme il l'est
/// dans [`ABONNEMENTS`] : sans cela la table dirait « Pro » et le repli
/// « Claude Ultra Plus », et l'égalité que ce commentaire revendique entre les
/// deux voies cesserait d'être vraie. L'élagage est le premier morceau
/// seulement, et jamais le dernier — un hypothétique `claude` nu resterait
/// « Claude » plutôt que de disparaître.
///
/// Les morceaux vides sont écartés, ce qui rend une chaîne de séparateurs seuls
/// à `None` du côté de [`libelle`] plutôt qu'à un libellé blanc.
fn embellir(identifiant: &str) -> String {
    let morceaux: Vec<&str> = identifiant
        .split('_')
        .filter(|morceau| !morceau.is_empty())
        .collect();

    // Les morceaux vides sont déjà partis : un reste non vide suffit donc à
    // garantir qu'élaguer ne laissera pas le libellé sans rien.
    let utiles = match morceaux.split_first() {
        Some((premier, reste)) if !reste.is_empty() && premier.to_lowercase() == "claude" => reste,
        _ => &morceaux[..],
    };

    utiles
        .iter()
        .map(|morceau| {
            let mut caracteres = morceau.chars();
            match caracteres.next() {
                Some(premier) => {
                    premier.to_uppercase().collect::<String>() + &caracteres.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Compose le libellé affiché à partir du type d'organisation et du palier de
/// limitation, ou `None` si le type ne dit rien.
///
/// Séparée de la lecture disque pour rester vérifiable : c'est ici que se
/// décident les libellés, et un test n'a pas à fabriquer un `~/.claude.json`
/// pour les exercer.
///
/// Le palier n'est joint qu'à un abonnement Max, seul cas où Claude Code lui-même
/// le fait — les deux valeurs de [`PALIERS_MAX`] ne se rencontrent d'ailleurs pas
/// ailleurs. Un palier inconnu, ou celui d'un compte Pro (`default_claude_ai`),
/// laisse le libellé nu : « Claude Max » sans chiffre reste juste, là où un
/// suffixe inventé ne le serait pas.
fn libelle(type_organisation: &str, palier: Option<&str>) -> Option<String> {
    let type_net = type_organisation.trim();
    if type_net.is_empty() {
        return None;
    }

    let base = ABONNEMENTS
        .iter()
        .find(|(cle, _)| *cle == type_net)
        .map(|(_, libelle)| (*libelle).to_string())
        .unwrap_or_else(|| embellir(type_net));

    if base.is_empty() {
        return None;
    }

    let suffixe = palier
        .map(str::trim)
        .filter(|_| type_net == "claude_max")
        .and_then(|palier| {
            PALIERS_MAX
                .iter()
                .find(|(cle, _)| *cle == palier)
                .map(|(_, suffixe)| *suffixe)
        });

    Some(match suffixe {
        Some(suffixe) => format!("{} {}", base, suffixe),
        None => base,
    })
}

/// Lit et analyse la configuration Claude Code, ou `None` si elle n'est pas
/// lisible.
///
/// Lue **une fois par lancement**, depuis le soir du 03/09/2026 : deux
/// segments s'y servent désormais — l'abonnement ici, et la fenêtre propre au
/// modèle dans [`crate::usage`] —, et le fichier fait 64 Ko. L'analyser deux
/// fois doublerait le seul surcoût mesurable du programme pour rien.
///
/// Rien n'y est fatal : fichier absent, JSON illisible — Claude Code le réécrit
/// en cours de session — rendent `None`, et chaque segment se retire.
pub(crate) fn lire_config() -> Option<Value> {
    let chemin = trouver_config()?;
    let texte = lire_texte(&chemin)?;
    serde_json::from_str(&texte).ok()
}

/// Lit l'abonnement du compte connecté dans la configuration déjà analysée, ou
/// `None` s'il n'est pas lisible.
///
/// Rien n'y est fatal, conformément à la règle générale du programme : champ
/// disparu d'une version à l'autre, valeur d'un autre type — chacun de ces
/// chemins retire le segment plutôt que d'inventer une valeur. Un abonnement
/// affiché de travers serait pire qu'un segment absent : c'est l'information la
/// moins volatile de la ligne, donc celle qu'on relit le moins.
pub(crate) fn abonnement(config: Option<&Value>) -> Option<String> {
    lire_compte(champ(config?, "oauthAccount"))
}

/// Compose le libellé à partir de l'objet `oauthAccount`, quel qu'en soit
/// l'état.
///
/// Extraite d'[`abonnement`] pour rester vérifiable sans détourner
/// `CLAUDE_STATUSLINE_CONFIG` : cette variable est unique au processus, et les
/// tests du crate s'exécutent en parallèle dans le même — l'un écraserait le
/// détournement de l'autre. La résolution du chemin, elle, est couverte par le
/// harnais de non-régression, qui lance un processus par cas.
///
/// Les deux champs ne sont retenus que sous forme de **chaîne**, là où
/// [`en_texte`] accepterait aussi un nombre ou un booléen : ce sont des
/// identifiants du contrat, et « 42 » ne nomme aucun abonnement. C'est aussi ce
/// que fait l'oracle PowerShell, dont l'égalité se vérifie cas par cas.
fn lire_compte(compte: &Value) -> Option<String> {
    let type_organisation = match champ(compte, "organizationType") {
        Value::String(texte) => texte.clone(),
        _ => return None,
    };

    let palier = match champ(compte, "organizationRateLimitTier") {
        Value::String(texte) => Some(texte.clone()),
        _ => None,
    };

    libelle(&type_organisation, palier.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_quatre_types_connus_ont_leur_libelle() {
        // Sans le « Claude » de tête depuis le 26/08/2026 : sept colonnes qui
        // ne distinguaient rien, dans la ligne de statut de Claude Code.
        assert_eq!(libelle("claude_pro", None).as_deref(), Some("Pro"));
        assert_eq!(libelle("claude_max", None).as_deref(), Some("Max"));
        assert_eq!(libelle("claude_team", None).as_deref(), Some("Team"));
        assert_eq!(
            libelle("claude_enterprise", None).as_deref(),
            Some("Enterprise")
        );
    }

    #[test]
    fn le_palier_ne_se_joint_qu_a_un_abonnement_max() {
        assert_eq!(
            libelle("claude_max", Some("default_claude_max_5x")).as_deref(),
            Some("Max 5x")
        );
        assert_eq!(
            libelle("claude_max", Some("default_claude_max_20x")).as_deref(),
            Some("Max 20x")
        );

        // Le palier d'un compte Pro ne dit rien du sien, et ne doit pas
        // s'accrocher au libellé d'un autre abonnement.
        assert_eq!(
            libelle("claude_pro", Some("default_claude_max_20x")).as_deref(),
            Some("Pro")
        );
        // Palier inconnu : le libellé reste nu plutôt que d'inventer un chiffre.
        assert_eq!(
            libelle("claude_max", Some("default_claude_ai")).as_deref(),
            Some("Max")
        );
    }

    #[test]
    fn un_type_hors_table_reste_lisible() {
        // Le jour où Claude Code ajoute un abonnement, la ligne l'affiche sans
        // qu'on ait rien à recompiler — et sans le « Claude » que la table
        // élague elle aussi, sans quoi les deux voies divergeraient.
        assert_eq!(
            libelle("claude_ultra_plus", None).as_deref(),
            Some("Ultra Plus")
        );
        assert_eq!(libelle("api", None).as_deref(), Some("Api"));
        // La casse d'origine n'est pas conservée : elle vient d'un identifiant,
        // pas d'un nom propre. Elle ne protège pas non plus de l'élagage.
        assert_eq!(libelle("CLAUDE_PRO", None).as_deref(), Some("Pro"));
        // Un « claude » nu n'est pas élagué : il ne reste rien derrière lui, et
        // un libellé vide retirerait le segment au lieu de le raccourcir.
        assert_eq!(libelle("claude", None).as_deref(), Some("Claude"));
    }

    #[test]
    fn un_type_vide_retire_le_segment() {
        assert_eq!(libelle("", None), None);
        assert_eq!(libelle("   ", None), None);
        // Des séparateurs seuls ne composent aucun libellé.
        assert_eq!(libelle("___", None), None);
    }

    /// Écrit une configuration jetable et rend son chemin.
    ///
    /// Le dossier porte le PID et l'étiquette du test, comme dans
    /// [`crate::capture`] : les tests s'exécutent en parallèle, et deux d'entre
    /// eux ne doivent pas se disputer un nom.
    fn config_jetable(etiquette: &str, contenu: &str) -> PathBuf {
        let dossier = std::env::temp_dir().join(format!(
            "statusline-test-{}-{}",
            std::process::id(),
            etiquette
        ));
        std::fs::create_dir_all(&dossier).expect("dossier jetable");

        let chemin = dossier.join(".claude.json");
        std::fs::write(&chemin, contenu).expect("configuration écrite");
        chemin
    }

    /// Lit l'abonnement d'une configuration donnée, sans toucher à
    /// l'environnement du processus. Voir [`lire_compte`] pour la raison.
    fn abonnement_de(chemin: &Path) -> Option<String> {
        let texte = lire_texte(chemin)?;
        let donnees: Value = serde_json::from_str(&texte).ok()?;
        lire_compte(champ(&donnees, "oauthAccount"))
    }

    #[test]
    fn le_champ_est_lu_dans_oauth_account() {
        let chemin = config_jetable(
            "abonnement-pro",
            r#"{"numStartups":42,"oauthAccount":{"emailAddress":"a@b.c",
                "organizationType":"claude_max",
                "organizationRateLimitTier":"default_claude_max_5x"}}"#,
        );

        assert_eq!(abonnement_de(&chemin).as_deref(), Some("Max 5x"));

        let _ = std::fs::remove_dir_all(chemin.parent().expect("dossier"));
    }

    #[test]
    fn une_configuration_muette_retire_le_segment() {
        // Configuration sans compte : le cas d'une session non connectée.
        let sans_compte = config_jetable("abonnement-sans-compte", r#"{"numStartups":42}"#);
        assert_eq!(abonnement_de(&sans_compte), None);
        let _ = std::fs::remove_dir_all(sans_compte.parent().expect("dossier"));

        // Compte présent, champ disparu — ce qu'une évolution du format ferait.
        let sans_type = config_jetable(
            "abonnement-sans-type",
            r#"{"oauthAccount":{"emailAddress":"a@b.c"}}"#,
        );
        assert_eq!(abonnement_de(&sans_type), None);
        let _ = std::fs::remove_dir_all(sans_type.parent().expect("dossier"));

        // JSON tronqué : le fichier est réécrit par Claude Code en cours de
        // session, et rien ne garantit qu'on ne le lise pas à mi-écriture.
        let tronque = config_jetable("abonnement-tronque", r#"{"oauthAccount":{"organizat"#);
        assert_eq!(abonnement_de(&tronque), None);
        let _ = std::fs::remove_dir_all(tronque.parent().expect("dossier"));

        // Fichier absent : ni panique, ni segment.
        assert_eq!(
            abonnement_de(Path::new("C:\\aucun-fichier-de-ce-nom-7b3e.json")),
            None
        );
    }

    #[test]
    fn un_type_qui_n_est_pas_une_chaine_ne_nomme_aucun_abonnement() {
        // « 42 » n'est pas un identifiant d'abonnement, et l'oracle PowerShell
        // le refuse de la même façon : c'est cette égalité que le harnais
        // vérifie cas par cas.
        assert_eq!(
            lire_compte(&serde_json::json!({"organizationType": 42})),
            None
        );
        assert_eq!(
            lire_compte(&serde_json::json!({"organizationType": true})),
            None
        );
        // Le palier non plus, mais lui n'emporte que le suffixe.
        assert_eq!(
            lire_compte(&serde_json::json!({
                "organizationType": "claude_max",
                "organizationRateLimitTier": 5
            }))
            .as_deref(),
            Some("Max")
        );
    }
}
