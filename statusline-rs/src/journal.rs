//! Journal de diagnostic — section 2 bis du script d'origine.
//!
//! Ce que ce journal répond, et rien d'autre : **la ligne a-t-elle disparu
//! parce que ce programme s'est tu, ou sans qu'il y soit pour rien ?**
//!
//! Claude Code pousse le résultat de chaque lancement tel quel, y compris
//! indéfini — la fonction `sz0` du binaire appelle `onResult(l)` avant même de
//! tester `l`. Trois causes produisent cet indéfini, et le programme n'en
//! contrôle qu'une : la sienne, un lancement qui échoue avant lui
//! (`spawn_failed`), un code de sortie non nul. Vu de l'interface, les trois se
//! ressemblent : la ligne s'efface jusqu'au rafraîchissement suivant.
//!
//! D'où la trace, une ligne par lancement : ce qui manque au journal désigne
//! exactement ce qui n'a jamais démarré. Un intervalle plus long que le
//! `refreshInterval` est un lancement perdu ; une cadence régulière et une ligne
//! jamais vide disculpent au contraire le programme, et renvoient le
//! clignotement au redessin de l'interface.
//!
//! Activation par **fichier témoin** plutôt que par variable d'environnement :
//! une variable ne se lit qu'au lancement de Claude Code, et le symptôme
//! s'observe dans la session déjà ouverte. Le témoin, lui, prend effet au
//! rafraîchissement suivant, et son retrait aussi. Il coûte un `metadata` sur un
//! disque local, quelques dizaines de microsecondes contre la quinzaine de
//! millisecondes du programme.
//!
//! # La panique, avec son lieu — 04/09/2026
//!
//! Le programme principal fait taire le hook de panique, pour que rien ne
//! sorte sur l'erreur standard. Le journal ne disait donc que « panic », sans
//! lieu ni cause, et retrouver la coupe fautive d'`analyser_iso` a demandé de
//! relire le code. Le hook retient désormais la dernière panique — fichier,
//! ligne, message — dans [`DERNIERE_PANIQUE`], et la ligne de journal la porte
//! sous `panique`, échappée en JSON. Le champ n'apparaît que sur un tir qui a
//! paniqué : `analyser-journal.ps1` lit les lignes par `ConvertFrom-Json`, et
//! un champ absent ne le gêne pas.

use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::sync::Mutex;

use crate::disque::chemin_etat;
use crate::largeur::largeur_console;
use crate::reglages::{NOM_JOURNAL, NOM_TEMOIN_JOURNAL};

/// Dernière panique du processus — lieu et message —, retenue par le hook pour
/// le journal. Un seul lancement, donc au plus une panique utile : la dernière
/// remplace la précédente.
static DERNIERE_PANIQUE: Mutex<Option<String>> = Mutex::new(None);

/// Retient une panique pour le journal, à partir de ce que le hook reçoit : le
/// lieu, s'il est connu, et la charge utile, dont le message se lit quand elle
/// est une chaîne — les deux formes que `panic!` produit.
///
/// Rien n'y est fatal : un verrou empoisonné, ce qui supposerait une panique
/// dans cette fonction même, laisse simplement la note absente.
pub(crate) fn noter_panique(
    lieu: Option<&std::panic::Location<'_>>,
    charge: &(dyn std::any::Any + Send),
) {
    let message = charge
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| charge.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "cause inconnue".to_string());

    let texte = match lieu {
        Some(l) => format!("{}:{} {}", l.file(), l.line(), message),
        None => message,
    };

    if let Ok(mut garde) = DERNIERE_PANIQUE.lock() {
        *garde = Some(texte);
    }
}

/// Rend la panique retenue, s'il y en a une.
fn panique_notee() -> Option<String> {
    DERNIERE_PANIQUE.lock().ok().and_then(|garde| garde.clone())
}

/// Échappe un texte pour le placer entre guillemets dans du JSON.
///
/// Le message d'une panique porte des chemins de fichiers, donc des barres
/// obliques inverses, et parfois des guillemets ou des retours à la ligne :
/// écrit tel quel, il romprait la ligne de journal que `ConvertFrom-Json`
/// doit relire.
fn echapper_json(texte: &str) -> String {
    let mut sortie = String::with_capacity(texte.len());
    for c in texte.chars() {
        match c {
            '"' => sortie.push_str("\\\""),
            '\\' => sortie.push_str("\\\\"),
            '\n' => sortie.push_str("\\n"),
            '\r' => sortie.push_str("\\r"),
            '\t' => sortie.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(sortie, "\\u{:04x}", c as u32);
            }
            c => sortie.push(c),
        }
    }
    sortie
}

/// Empreinte FNV-1a 64 bits, pour reconnaître une ligne sans la recopier.
///
/// Le journal note ce qui a changé d'un lancement à l'autre, pas ce qui est
/// affiché : la ligne porte le répertoire de travail, et un fichier de
/// diagnostic n'a pas à en garder l'historique.
fn empreinte(texte: &str) -> u64 {
    let mut somme: u64 = 0xcbf2_9ce4_8422_2325;
    for octet in texte.as_bytes() {
        somme ^= u64::from(*octet);
        somme = somme.wrapping_mul(0x0000_0100_0000_01b3);
    }
    somme
}

/// Compose la ligne de journal, en JSON compact.
///
/// Séparée de l'écriture pour rester vérifiable par un test : c'est le format
/// que lit `analyser-journal.ps1`.
///
/// `voie` nomme le chemin emprunté dans le programme principal — `ok` pour
/// l'assemblage complet, un libellé de repli sinon. C'est lui qui distingue une
/// ligne courte d'une ligne dégradée. `panique`, quand elle existe, dit où et
/// pourquoi : elle n'accompagne que la voie du même nom.
fn ligne_journal(
    instant_ms: u128,
    duree_us: u128,
    voie: &str,
    ligne: &str,
    largeur: Option<u16>,
    panique: Option<&str>,
) -> String {
    let mut texte = format!(
        r#"{{"t":{},"us":{},"pid":{},"voie":"{}","n":{},"sig":"{:016x}""#,
        instant_ms,
        duree_us,
        std::process::id(),
        voie,
        ligne.chars().count(),
        empreinte(ligne)
    );
    // Largeur de la console vue par le processus — 19/09/2026 —, `null` s'il
    // n'en a pas : c'est la mesure qui dit si le repli en largeur a de quoi
    // travailler depuis un lancement réel par Claude Code.
    match largeur {
        Some(l) => {
            let _ = write!(texte, r#","larg":{}"#, l);
        }
        None => texte.push_str(r#","larg":null"#),
    }
    if let Some(panique) = panique {
        let _ = write!(texte, r#","panique":"{}""#, echapper_json(panique));
    }
    texte.push('}');
    texte
}

/// Indique si le journal est demandé, par la présence du fichier témoin.
fn journal_demande() -> bool {
    chemin_etat(NOM_TEMOIN_JOURNAL)
        .map(|c| c.exists())
        .unwrap_or(false)
}

/// Ajoute une ligne au journal, si le témoin est présent.
///
/// Appelée **après** l'écriture de la ligne de statut : le diagnostic ne doit
/// pas retarder l'affichage qu'il observe. Rien n'y est fatal, conformément à
/// la règle générale du programme — un journal qui échoue se tait.
pub(crate) fn journaliser(duree: std::time::Duration, voie: &str, ligne: &str) {
    if !journal_demande() {
        return;
    }
    let Some(chemin) = chemin_etat(NOM_JOURNAL) else {
        return;
    };
    if let Some(dossier) = chemin.parent() {
        let _ = std::fs::create_dir_all(dossier);
    }

    let instant_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    // Ouverture en ajout : plusieurs lancements peuvent se chevaucher quand
    // Claude Code relance la ligne avant d'avoir tué le tir précédent.
    if let Ok(mut fichier) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&chemin)
    {
        let _ = writeln!(
            fichier,
            "{}",
            ligne_journal(
                instant_ms,
                duree.as_micros(),
                voie,
                ligne,
                largeur_console(),
                panique_notee().as_deref()
            )
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empreinte_conforme_a_fnv1a() {
        // Vecteurs de référence de FNV-1a 64 bits.
        assert_eq!(empreinte(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(empreinte("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(empreinte("foobar"), 0x8594_4171_f739_67e8);
        // Deux lignes voisines doivent se distinguer.
        assert_ne!(empreinte("ctx  34%"), empreinte("ctx  35%"));
    }

    #[test]
    fn ligne_journal_est_du_json_compact() {
        let ligne = ligne_journal(1_787_324_661_000, 12_480, "ok", "Opus 5", Some(120), None);
        assert!(ligne.starts_with(r#"{"t":1787324661000,"us":12480,"pid":"#));
        assert!(ligne.contains(r#""voie":"ok""#));
        assert!(ligne.contains(r#""n":6"#));
        assert!(ligne.contains(&format!(r#""sig":"{:016x}""#, empreinte("Opus 5"))));
        assert!(ligne.ends_with('}'));
        // Sans panique, pas de champ : la forme des tirs ordinaires ne change
        // pas.
        assert!(!ligne.contains("panique"));
        // Les caractères multi-octets se comptent en caractères, pas en octets.
        assert!(ligne_journal(0, 0, "ok", "données · été", None, None).contains(r#""n":13"#));
    }

    /// La panique s'écrit en dernier champ, échappée : un chemin Windows et
    /// des guillemets ne doivent pas rompre la ligne.
    #[test]
    fn la_panique_est_journalisee_echappee() {
        let ligne = ligne_journal(
            0,
            0,
            "panic",
            "Opus 5",
            None,
            Some("src\\x.rs:12 index 4 \"hors\" bornes\n"),
        );
        assert!(
            ligne.ends_with(r#","panique":"src\\x.rs:12 index 4 \"hors\" bornes\n"}"#),
            "{ligne}"
        );
        assert_eq!(
            echapper_json("a\"b\\c\u{1}\té"),
            concat!(r#"a\"b\\c"#, "\\", "u0001", r#"\té"#)
        );
    }

    /// Le hook retient lieu et message, sous les deux formes que `panic!`
    /// produit, et la dernière panique remplace la précédente.
    #[test]
    fn la_derniere_panique_est_retenue_avec_son_lieu() {
        let charge: Box<dyn std::any::Any + Send> = Box::new("boum");
        noter_panique(Some(std::panic::Location::caller()), charge.as_ref());
        let notee = panique_notee().expect("panique notée");
        assert!(notee.contains("journal.rs:"), "{notee}");
        assert!(notee.ends_with(" boum"), "{notee}");

        // Message possédé — celui de `panic!("{}", x)`.
        let charge: Box<dyn std::any::Any + Send> = Box::new(String::from("re-boum"));
        noter_panique(None, charge.as_ref());
        assert_eq!(panique_notee().as_deref(), Some("re-boum"));

        // Charge d'un autre type : la note dit au moins qu'on ne sait pas.
        let charge: Box<dyn std::any::Any + Send> = Box::new(42_u8);
        noter_panique(None, charge.as_ref());
        assert_eq!(panique_notee().as_deref(), Some("cause inconnue"));
    }
}
