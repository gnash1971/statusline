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
///
/// # Lecture bornée — 20/09/2026
///
/// CodeQL (`rust/access-invalid-pointer`) signalait la lecture à travers le
/// pointeur que `VerQueryValueW` rend : l'analyseur ne connaît pas le contrat
/// de l'API — un pointeur dans le bloc qu'on lui a passé — et voit un pointeur
/// né nul, passé à une fonction opaque, puis déréférencé. Plutôt que d'écarter
/// l'alerte, les lectures ne passent plus par ce pointeur : il n'est utilisé
/// que comme **adresse**, convertie en décalage depuis le début du bloc
/// ([`decalage_dans_bloc`]), et les octets sont lus par indexation bornée dans
/// le `Vec<u32>` lui-même ([`mot_a`]). Plus aucun `unsafe` à la lecture, et une
/// adresse qui sortirait du bloc — ce que l'API ne fait pas — rend `None` au
/// lieu d'être suivie.
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
    // Le pointeur rendu n'est qu'une adresse dans `bloc` : les deux mots se
    // lisent par indexation bornée, à partir de son décalage.
    let debut = decalage_dans_bloc(&bloc, trad)?;
    let langue = mot_a(&bloc, debut)?;
    let page = mot_a(&bloc, debut + 2)?;

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
    // compris. La chaîne est copiée mot par mot jusqu'au premier zéro, sans
    // jamais sortir du bloc : une longueur qui le dépasserait tronque la
    // copie, elle ne lit pas au-delà.
    let debut = decalage_dans_bloc(&bloc, valeur)?;
    let brut: Vec<u16> = (0..longueur as usize)
        .map_while(|i| mot_a(&bloc, debut + 2 * i))
        .take_while(|&c| c != 0)
        .collect();
    Some(String::from_utf16_lossy(&brut))
}

/// Décalage, en octets depuis le début du bloc, de l'adresse que
/// `VerQueryValueW` a rendue — ou `None` si elle tombe hors du bloc.
///
/// L'API désigne toujours l'intérieur du bloc qu'on lui a passé ; la borne
/// n'est là que pour que rien ne dépende de cette promesse. Le pointeur n'est
/// jamais déréférencé : seule son adresse est comparée à celle du bloc.
fn decalage_dans_bloc(bloc: &[u32], adresse: *const core::ffi::c_void) -> Option<usize> {
    let debut = bloc.as_ptr() as usize;
    let decalage = (adresse as usize).checked_sub(debut)?;
    (decalage < std::mem::size_of_val(bloc)).then_some(decalage)
}

/// Octet du bloc au décalage donné, ou `None` s'il déborde.
///
/// Le bloc est tenu en mots de 32 bits pour l'alignement ; l'octet se prend
/// dans la représentation mémoire du mot (`to_ne_bytes`), ce qui rend la
/// lecture identique à ce que l'API a écrit, quel que soit le boutisme.
fn octet_a(bloc: &[u32], decalage: usize) -> Option<u8> {
    Some(bloc.get(decalage / 4)?.to_ne_bytes()[decalage % 4])
}

/// Mot UTF-16 du bloc qui commence au décalage donné, ou `None` s'il déborde.
///
/// Les deux octets peuvent chevaucher deux mots de 32 bits : chacun est pris
/// séparément, sans exigence d'alignement.
fn mot_a(bloc: &[u32], decalage: usize) -> Option<u16> {
    let bas = octet_a(bloc, decalage)?;
    let haut = octet_a(bloc, decalage.checked_add(1)?)?;
    Some(u16::from_ne_bytes([bas, haut]))
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

    /// Les lectures bornées dans le bloc en mots de 32 bits : octet et mot
    /// UTF-16 à tout décalage, chevauchement de deux mots compris, et `None`
    /// dès que la lecture déborde — jamais une panique.
    #[test]
    fn le_bloc_se_lit_par_indexation_bornee() {
        // Octets 1..=8 en mémoire, quel que soit le boutisme.
        let bloc = [
            u32::from_ne_bytes([1, 2, 3, 4]),
            u32::from_ne_bytes([5, 6, 7, 8]),
        ];
        assert_eq!(octet_a(&bloc, 0), Some(1));
        assert_eq!(octet_a(&bloc, 3), Some(4));
        assert_eq!(octet_a(&bloc, 4), Some(5));
        assert_eq!(octet_a(&bloc, 7), Some(8));
        assert_eq!(octet_a(&bloc, 8), None);

        assert_eq!(mot_a(&bloc, 0), Some(u16::from_ne_bytes([1, 2])));
        // À cheval sur les deux mots de 32 bits.
        assert_eq!(mot_a(&bloc, 3), Some(u16::from_ne_bytes([4, 5])));
        assert_eq!(mot_a(&bloc, 6), Some(u16::from_ne_bytes([7, 8])));
        // Le second octet déborderait.
        assert_eq!(mot_a(&bloc, 7), None);
        assert_eq!(mot_a(&bloc, usize::MAX), None);
        assert_eq!(mot_a(&[], 0), None);
    }

    /// Le pointeur rendu par l'API n'est qu'une adresse : dans le bloc, elle
    /// devient un décalage ; avant ou après lui, `None`. Les pointeurs sont
    /// fabriqués par arithmétique enveloppante et jamais déréférencés.
    #[test]
    fn une_adresse_hors_du_bloc_ne_donne_aucun_decalage() {
        let bloc = [0u32; 3];
        let base = bloc.as_ptr().cast::<u8>();
        let dans = |octets: usize| base.wrapping_add(octets).cast::<core::ffi::c_void>();
        assert_eq!(decalage_dans_bloc(&bloc, dans(0)), Some(0));
        assert_eq!(decalage_dans_bloc(&bloc, dans(5)), Some(5));
        assert_eq!(decalage_dans_bloc(&bloc, dans(11)), Some(11));
        // Premier octet après le bloc, puis un octet avant.
        assert_eq!(decalage_dans_bloc(&bloc, dans(12)), None);
        assert_eq!(
            decalage_dans_bloc(&bloc, base.wrapping_sub(1).cast::<core::ffi::c_void>()),
            None
        );
        assert_eq!(decalage_dans_bloc(&bloc, std::ptr::null()), None);
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
