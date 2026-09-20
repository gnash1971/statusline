//! Version du binaire Claude Code installé — seconde moitié de la section 5.
//!
//! Même contrainte que pour la branche (voir [`crate::depot`]) : la ligne se
//! rafraîchit souvent, et lancer un processus à chaque fois se paierait cher.
//! La version est donc prise dans les métadonnées du fichier, jamais en lançant
//! « claude --version » — le binaire pèse plus de 300 Mo.

use std::path::{Path, PathBuf};

use crate::reglages::CHEMIN_BINAIRE_RELATIF;

/// Rend le chemin du binaire Claude Code, ou `None` s'il reste introuvable.
///
/// `CLAUDE_STATUSLINE_BINAIRE` l'emporte, et sans repli : une désignation
/// explicite qui ne résout pas doit se voir, pas se faire remplacer en silence
/// par l'emplacement habituel.
///
/// Le `PATH` n'est volontairement pas balayé en dernier recours : le coût
/// serait payé à chaque rafraîchissement, pour un gain nul tant que
/// l'installation est standard — et le segment se retire proprement sinon.
fn trouver_binaire() -> Option<PathBuf> {
    if let Ok(explicite) = std::env::var("CLAUDE_STATUSLINE_BINAIRE") {
        if !explicite.is_empty() {
            let chemin = PathBuf::from(explicite);
            return if chemin.is_file() { Some(chemin) } else { None };
        }
    }

    let profil = std::env::var("USERPROFILE")
        .ok()
        .filter(|v| !v.is_empty())?;
    let natif = Path::new(&profil).join(CHEMIN_BINAIRE_RELATIF);
    if natif.is_file() {
        Some(natif)
    } else {
        None
    }
}

/// Encode une chaîne en UTF-16 terminée par zéro, pour les API Win32.
fn large(texte: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(texte)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Rend la chaîne `ProductVersion` du bloc `StringFileInfo` d'un exécutable.
///
/// C'est bien ce bloc, et non la structure numérique `VS_FIXEDFILEINFO`, que
/// lit `FileVersionInfo.ProductVersion` côté .NET : les deux peuvent diverger,
/// et le script d'origine s'appuie sur la chaîne. La langue et la page de codes
/// sont d'abord relues dans le fichier plutôt que supposées.
///
/// # Alignement — 04/09/2026
///
/// Le bloc est alloué en mots de 32 bits, et non en octets. Les structures du
/// bloc de version sont alignées sur 32 bits par rapport à son début, mais un
/// `Vec<u8>` ne garantit qu'un alignement de 1 : lire un `u16` à travers un
/// pointeur qui y mène était indéfini en théorie, même si le tas de Windows
/// aligne à 16 en pratique. Les lectures passent en outre par
/// `read_unaligned`, qui ne suppose rien, et la chaîne est copiée mot par mot
/// plutôt que vue en place par `from_raw_parts`, qui exige lui aussi
/// l'alignement.
fn version_produit(chemin: &Path) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };

    let fichier = large(&chemin.to_string_lossy());

    // SAFETY : `fichier` est terminé par zéro, et le second paramètre — un
    // handle que l'API n'utilise pas — accepte le pointeur nul.
    let taille = unsafe { GetFileVersionInfoSizeW(fichier.as_ptr(), std::ptr::null_mut()) };
    if taille == 0 {
        return None;
    }

    let mut bloc = vec![0u32; (taille as usize).div_ceil(4)];
    // SAFETY : `bloc` fait au moins `taille` octets, et l'API n'écrit pas
    // au-delà de la longueur qu'on lui passe.
    let ecrit =
        unsafe { GetFileVersionInfoW(fichier.as_ptr(), 0, taille, bloc.as_mut_ptr().cast()) };
    if ecrit == 0 {
        return None;
    }

    // Langue et page de codes effectivement présentes dans le fichier.
    let mut trad: *mut core::ffi::c_void = std::ptr::null_mut();
    let mut n: u32 = 0;
    let cle = large("\\VarFileInfo\\Translation");
    // SAFETY : `bloc` est un bloc de version valide, rempli à l'instant et
    // vivant jusqu'à la fin de la fonction ; la clé est terminée par zéro ; les
    // deux sorties sont des variables locales que l'API remplit.
    let trouve = unsafe { VerQueryValueW(bloc.as_ptr().cast(), cle.as_ptr(), &mut trad, &mut n) };
    if trouve == 0 || n < 4 || trad.is_null() {
        return None;
    }
    // SAFETY : l'API garantit au moins `n` octets lisibles derrière `trad`, et
    // `n >= 4` couvre les deux `u16` lus ; `read_unaligned` ne suppose aucun
    // alignement.
    let (langue, page) = unsafe {
        let mots = trad.cast::<u16>();
        (mots.read_unaligned(), mots.add(1).read_unaligned())
    };

    let sous_bloc = format!(
        "\\StringFileInfo\\{:04x}{:04x}\\ProductVersion",
        langue, page
    );
    let cle = large(&sous_bloc);
    let mut valeur: *mut core::ffi::c_void = std::ptr::null_mut();
    let mut longueur: u32 = 0;
    // SAFETY : mêmes garanties que l'appel précédent, sur le même bloc.
    let trouve = unsafe {
        VerQueryValueW(
            bloc.as_ptr().cast(),
            cle.as_ptr(),
            &mut valeur,
            &mut longueur,
        )
    };
    if trouve == 0 || longueur == 0 || valeur.is_null() {
        return None;
    }

    // Pour une chaîne, `longueur` compte des caractères UTF-16, terminateur
    // compris. La chaîne est copiée mot par mot jusqu'au premier zéro.
    let mots = valeur.cast::<u16>();
    let brut: Vec<u16> = (0..longueur as usize)
        // SAFETY : l'API garantit `longueur` caractères lisibles derrière
        // `valeur`, et l'indice reste dans cette borne.
        .map(|i| unsafe { mots.add(i).read_unaligned() })
        .take_while(|&c| c != 0)
        .collect();
    Some(String::from_utf16_lossy(&brut))
}

/// Ramène une chaîne de version Windows à l'écriture de Claude Code.
///
/// Windows y stocke quatre composantes (« 2.1.238.0 ») là où Claude Code en
/// annonce trois. La quatrième, toujours nulle, est retirée ; une écriture
/// inattendue est rendue telle quelle plutôt que perdue, et une chaîne blanche
/// rend `None`.
///
/// Séparée de [`version_binaire`] pour rester vérifiable : la fonction complète
/// lit le disque du poste, quand la règle, elle, ne dépend que d'une chaîne.
fn normaliser_version(version: &str) -> Option<String> {
    let version = version.trim();
    if version.is_empty() {
        return None;
    }

    if let Some(tronquee) = version.strip_suffix(".0") {
        let morceaux: Vec<&str> = tronquee.split('.').collect();
        if morceaux.len() == 3
            && morceaux
                .iter()
                .all(|m| !m.is_empty() && m.chars().all(|c| c.is_ascii_digit()))
        {
            return Some(tronquee.to_string());
        }
    }

    Some(version.to_string())
}

/// Lit la version du binaire installé, ou `None` si elle n'est pas lisible.
///
/// La version est prise dans les métadonnées du fichier, jamais en lançant
/// « claude --version » : le binaire pèse plus de 300 Mo, et le démarrer à
/// chaque rafraîchissement est hors de question. Voir [`normaliser_version`]
/// pour ce qui est fait de la chaîne lue.
pub(crate) fn version_binaire() -> Option<String> {
    let chemin = trouver_binaire()?;
    normaliser_version(&version_produit(&chemin)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_quatrieme_composante_nulle_est_retiree() {
        assert_eq!(normaliser_version("2.1.238.0").as_deref(), Some("2.1.238"));
        assert_eq!(
            normaliser_version(" 2.1.260.0 ").as_deref(),
            Some("2.1.260")
        );
        // Quatrième composante non nulle, ou écriture inattendue : tel quel.
        assert_eq!(
            normaliser_version("2.1.238.7").as_deref(),
            Some("2.1.238.7")
        );
        assert_eq!(normaliser_version("2.1.0").as_deref(), Some("2.1.0"));
        assert_eq!(normaliser_version("2.1.x.0").as_deref(), Some("2.1.x.0"));
        assert_eq!(normaliser_version("beta.0").as_deref(), Some("beta.0"));
        assert_eq!(normaliser_version("   "), None);
    }

    /// Lecture réelle d'un bloc de version, sur une bibliothèque système
    /// présente sur tout Windows : c'est l'aller-retour qui vérifie la relecture
    /// de la traduction et la copie de la chaîne, alignement compris. Le
    /// harnais couvre, lui, la lecture de `claude.exe`.
    #[test]
    fn le_bloc_de_version_d_une_bibliotheque_systeme_se_lit() {
        let racine = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        let kernel = Path::new(&racine).join("System32").join("kernel32.dll");
        if !kernel.is_file() {
            // Poste sans System32 accessible : rien à vérifier ici.
            return;
        }
        let version = version_produit(&kernel).expect("ProductVersion de kernel32");
        assert!(!version.trim().is_empty());
        assert!(version.chars().any(|c| c.is_ascii_digit()));

        // Fichier sans bloc de version, ou absent : `None`, jamais une panique.
        let sans_bloc = Path::new(&racine).join("win.ini");
        if sans_bloc.is_file() {
            assert_eq!(version_produit(&sans_bloc), None);
        }
        assert_eq!(
            version_produit(Path::new("C:\\aucun-fichier-de-ce-nom-9c2e.exe")),
            None
        );
    }
}
