//! Branche Git courante, lue sur le disque — première moitié de la section 5.
//!
//! La branche manque au contrat d'entrée et se lit donc sur le disque — comme
//! la version installée jusqu'au retrait de son segment, le 23/09/2026. La même
//! contrainte gouverne : la ligne se rafraîchit souvent, et lancer un processus
//! à chaque fois se paierait cher. Tout passe par des lectures de fichier.
//!
//! « worktree.branch » n'est renseigné que pour les sessions lancées avec
//! `--worktree`. La branche est donc lue directement dans « .git/HEAD », plutôt
//! qu'en appelant « git », ce qui aurait en outre supposé git présent dans le
//! `PATH`.

use std::path::{Path, PathBuf};

use crate::disque::lire_texte;
use crate::reglages::REMONTEE_MAX_GIT;

/// Remonte les dossiers parents jusqu'au premier « .git », et rend son chemin,
/// ou `None` hors dépôt.
fn trouver_git(depart: &Path) -> Option<PathBuf> {
    let mut courant = Some(depart);

    for _ in 0..REMONTEE_MAX_GIT {
        let dossier = courant?;
        let candidat = dossier.join(".git");
        if candidat.exists() {
            return Some(candidat);
        }
        courant = dossier.parent();
    }

    None
}

/// Rend le dossier « .git » réel derrière un chemin trouvé, ou `None`.
///
/// Les deux formes sont gérées : le dossier habituel, et le fichier
/// « gitdir: <chemin> » que posent les worktrees liés et les sous-modules.
fn resoudre_dossier_git(chemin: &Path) -> Option<PathBuf> {
    if chemin.is_dir() {
        return Some(chemin.to_path_buf());
    }

    let contenu = lire_texte(chemin)?;
    let cible = contenu
        .lines()
        .find_map(|ligne| ligne.trim_start().strip_prefix("gitdir:"))?
        .trim()
        .to_string();

    let cible = Path::new(&cible);
    if cible.is_absolute() {
        return Some(cible.to_path_buf());
    }
    Some(chemin.parent()?.join(cible))
}

/// Lit le nom de branche dans un dossier « .git », ou `None`.
fn lire_branche_head(dossier_git: &Path) -> Option<String> {
    let head = dossier_git.join("HEAD");
    if !head.exists() {
        return None;
    }

    let contenu = lire_texte(&head)?;
    let contenu = contenu.trim();

    if let Some(reste) = contenu.strip_prefix("ref:") {
        let reste = reste.trim_start();
        if let Some(branche) = reste.strip_prefix("refs/heads/") {
            let branche = branche.trim();
            if !branche.is_empty() {
                return Some(branche.to_string());
            }
        }
        return None;
    }

    // HEAD détachée : le SHA abrégé situe mieux qu'un segment absent.
    let longueur = contenu.len();
    if (7..=40).contains(&longueur) && contenu.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(contenu[..7].to_string());
    }

    None
}

/// Renvoie la branche Git couvrant un répertoire, ou `None` hors dépôt.
pub(crate) fn branche_git(depart: &str) -> Option<String> {
    if depart.trim().is_empty() {
        return None;
    }
    let depart = Path::new(depart);
    if !depart.exists() {
        return None;
    }

    let chemin = trouver_git(depart)?;
    let dossier = resoudre_dossier_git(&chemin)?;
    lire_branche_head(&dossier)
}
