//! Capture du payload — le contrat d'entrée relevé, plutôt que déduit.
//!
//! Ce que cette capture répond, et rien d'autre : **que Claude Code envoie-t-il
//! réellement à la ligne de statut, dans la version installée aujourd'hui ?**
//!
//! La question n'est pas rhétorique. Le module principal documente un contrat
//! d'entrée, mais ce contrat a toujours été obtenu de deux façons également
//! indirectes : la documentation publique, qui retarde sur le binaire, et le
//! scan de `claude.exe` — 384 Mo de bundle JavaScript en clair, plusieurs
//! minutes par motif cherché, des identifiants minifiés à suivre de proche en
//! proche, et aucune garantie d'avoir tout vu. Les champs apparus depuis
//! 2.1.226 ont été découverts ainsi, un par un, et avec eux l'incertitude de
//! ceux qu'on n'a pas cherchés.
//!
//! Un payload relevé sur le poste tranche en une seconde ce que ces deux voies
//! approchent. Il a trois usages :
//!
//!   - **vérifier** le contrat après chaque mise à jour de Claude Code, avant
//!     de faire dépendre un segment d'un champ ;
//!   - **rejouer** un affichage : le fichier produit est le JSON tel qu'il est
//!     arrivé, donc réinjectable au binaire (`Get-Content … | statusline.exe`)
//!     ou versable au harnais de non-régression comme nouveau cas ;
//!   - **diagnostiquer** un payload que le programme n'a pas su lire, cas où
//!     c'est précisément le contenu brut qui manque.
//!
//! # Armement, et pourquoi le témoin est consommé
//!
//! Comme le journal, la capture s'arme par **fichier témoin** et non par
//! variable d'environnement : une variable ne se lit qu'au lancement de Claude
//! Code, alors que le besoin naît dans la session déjà ouverte. Le témoin prend
//! effet au rafraîchissement suivant.
//!
//! Là où le journal note chaque lancement, la capture n'en relève **qu'un** :
//! le témoin est retiré dès que le fichier est écrit. Avec un rafraîchissement
//! par seconde, un témoin persistant réécrirait le même payload indéfiniment,
//! pour rien — les champs ne changent qu'à la marge d'un tir à l'autre. Le
//! retrait sert aussi d'accusé de réception : le témoin encore là signifie que
//! rien n'a été capturé, et qu'il faut chercher pourquoi.
//!
//! Deux situations laissent donc le témoin en place, délibérément : un payload
//! blanc — il n'y a rien à relever, et le tube rompu est déjà la réponse du
//! journal — et une écriture qui échoue. Dans les deux cas le tir suivant
//! réessaie.
//!
//! # Ce que le fichier contient
//!
//! Le payload **brut**, tel qu'il est arrivé sur l'entrée standard, sans
//! reformatage ni filtrage. C'est la seule forme qui réponde à la question
//! posée : un JSON réindenté par le programme ne dirait plus rien de ce qu'il a
//! reçu, et ne se rejouerait pas à l'identique.
//!
//! Brut jusqu'à l'octet, depuis le 04/09/2026 : le payload est reçu en octets
//! et relevé tel quel, avant toute conversion en texte. Une entrée qui ne
//! serait pas de l'UTF-8 valide est précisément celle qu'on voudrait voir, et
//! elle était jusque-là la seule que la capture ne pouvait pas relever, la
//! lecture de l'entrée standard échouant avant elle.
//!
//! Il porte donc aussi les chemins de travail de la session, son identifiant et
//! le nom du projet. Il vit pour cette raison sous `%LOCALAPPDATA%`, jamais
//! dans le workspace, exactement comme le journal.

use std::path::Path;

use crate::disque::{chemin_etat, ecrire_atomique};
use crate::reglages::{NOM_CAPTURE, NOM_TEMOIN_CAPTURE};

/// Relève le payload si la capture est armée.
///
/// Appelée **après** l'écriture de la ligne de statut : le diagnostic ne doit
/// pas retarder l'affichage qu'il observe. Rien n'y est fatal, conformément à
/// la règle générale du programme.
///
/// Le coût en régime ordinaire — celui de tous les rafraîchissements, la
/// capture n'étant armée que quelques secondes dans la vie d'un poste — est
/// l'interrogation d'un chemin absent sur un disque local : quelques dizaines
/// de microsecondes, contre la douzaine de millisecondes du programme.
pub(crate) fn capturer(payload: &[u8]) {
    let (Some(temoin), Some(sortie)) = (chemin_etat(NOM_TEMOIN_CAPTURE), chemin_etat(NOM_CAPTURE))
    else {
        return;
    };

    capturer_vers(&temoin, &sortie, payload);
}

/// Cœur de [`capturer`], les deux chemins passés en paramètre plutôt que
/// résolus depuis l'environnement : la capture devient ainsi vérifiable sans
/// détourner `%LOCALAPPDATA%`, qu'un test modifierait pour tous les autres —
/// les tests s'exécutant en parallèle.
///
/// Rend `true` si un payload a été relevé, ce dont seuls les tests se servent :
/// l'appelant, lui, n'a rien à décider de ce résultat.
fn capturer_vers(temoin: &Path, sortie: &Path, payload: &[u8]) -> bool {
    if !temoin.exists() {
        return false;
    }

    // Un payload blanc ne se capture pas : le fichier produit ne dirait rien, et
    // consommer le témoin ferait passer ce non-relevé pour un relevé.
    if payload.iter().all(u8::is_ascii_whitespace) {
        return false;
    }

    if !ecrire_atomique(sortie, payload) {
        return false;
    }

    // Le retrait est ce qui borne la capture à un seul payload. Son échec ne
    // change rien à ce qui a été écrit : le tir suivant réécrira le même
    // contenu, ce qui est sans conséquence.
    let _ = std::fs::remove_file(temoin);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prépare un dossier jetable et rend le couple (témoin, sortie).
    ///
    /// Le dossier porte le PID et l'étiquette du test, comme dans
    /// [`crate::cache`] : les tests s'exécutent en parallèle, et deux d'entre
    /// eux ne doivent pas se disputer un nom.
    fn bac(etiquette: &str, avec_temoin: bool) -> (std::path::PathBuf, std::path::PathBuf) {
        let dossier = std::env::temp_dir().join(format!(
            "statusline-test-{}-{}",
            std::process::id(),
            etiquette
        ));
        std::fs::create_dir_all(&dossier).expect("dossier jetable");

        let temoin = dossier.join(NOM_TEMOIN_CAPTURE);
        if avec_temoin {
            std::fs::write(&temoin, b"").expect("témoin posé");
        }

        (temoin, dossier.join(NOM_CAPTURE))
    }

    /// Retire le dossier jetable d'un test.
    fn nettoyer(chemin: &Path) {
        if let Some(dossier) = chemin.parent() {
            let _ = std::fs::remove_dir_all(dossier);
        }
    }

    #[test]
    fn payload_releve_tel_quel_et_temoin_consomme() {
        let (temoin, sortie) = bac("capture-armee", true);
        let payload = r#"{"model":{"display_name":"Opus 5"},"cwd":"C:\\projet"}"#;

        assert!(capturer_vers(&temoin, &sortie, payload.as_bytes()));

        // Le fichier porte le payload à l'octet près : c'est ce qui le rend
        // rejouable.
        assert_eq!(std::fs::read_to_string(&sortie).expect("relu"), payload);
        // Le témoin consommé borne la capture à un seul payload.
        assert!(!temoin.exists());

        nettoyer(&temoin);
    }

    #[test]
    fn sans_temoin_rien_n_est_ecrit() {
        let (temoin, sortie) = bac("capture-desarmee", false);

        assert!(!capturer_vers(&temoin, &sortie, br#"{"model":{}}"#));
        assert!(!sortie.exists());

        nettoyer(&temoin);
    }

    #[test]
    fn payload_blanc_conserve_le_temoin() {
        let (temoin, sortie) = bac("capture-blanche", true);

        assert!(!capturer_vers(&temoin, &sortie, b"   \n"));
        assert!(!sortie.exists());
        // Le témoin reste armé : le tir suivant aura peut-être un payload.
        assert!(temoin.exists());

        nettoyer(&temoin);
    }

    #[test]
    fn payload_illisible_est_capture_quand_meme() {
        // C'est le cas qui motive la capture : le programme n'a pas su lire ce
        // JSON, et seul son contenu brut dira pourquoi.
        let (temoin, sortie) = bac("capture-illisible", true);
        let tronque = r#"{"model":{"display_na"#;

        assert!(capturer_vers(&temoin, &sortie, tronque.as_bytes()));
        assert_eq!(std::fs::read_to_string(&sortie).expect("relu"), tronque);

        nettoyer(&temoin);
    }

    /// Un payload qui n'est pas de l'UTF-8 valide est relevé octet pour octet :
    /// c'est le cas où le contenu brut est tout ce qu'on a.
    #[test]
    fn payload_hors_utf8_est_capture_tel_quel() {
        let (temoin, sortie) = bac("capture-octets", true);
        let octets: &[u8] = b"{\"model\":{\"display_name\":\"Op\xffus\"}}";

        assert!(capturer_vers(&temoin, &sortie, octets));
        assert_eq!(std::fs::read(&sortie).expect("relu"), octets);

        nettoyer(&temoin);
    }
}
