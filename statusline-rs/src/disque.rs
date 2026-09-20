//! Accès disque partagés : racine des fichiers d'état, lecture d'un texte,
//! écriture atomique.
//!
//! Ces fonctions viennent de la section 4 du script d'origine, mais plusieurs
//! modules s'en servent — [`crate::journal`], [`crate::cache`],
//! [`crate::capture`] et [`crate::depot`]. Les loger ici plutôt que dans l'un
//! d'eux évite de faire dépendre le journal du cache pour un simple
//! `Path::join`.

use std::path::{Path, PathBuf};

use crate::reglages::DOSSIER_ETAT;

/// Chemin d'un fichier d'état, ou `None` si aucune racine n'est disponible.
///
/// Le repli sur `%TEMP%` existe pour le harnais de non-régression, qui détourne
/// la racine vers un dossier jetable afin de ne pas toucher au cache réel.
pub(crate) fn chemin_etat(nom: &str) -> Option<PathBuf> {
    let racine = std::env::var("LOCALAPPDATA")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("TEMP").ok().filter(|v| !v.is_empty()))?;

    Some(Path::new(&racine).join(DOSSIER_ETAT).join(nom))
}

/// Lit un fichier texte en UTF-8, BOM retiré, ou `None` s'il est illisible.
pub(crate) fn lire_texte(chemin: &Path) -> Option<String> {
    lire_texte_resultat(chemin).ok()
}

/// Comme [`lire_texte`], mais rend l'erreur au lieu de l'écraser.
///
/// Cette variante existe pour [`crate::cache::lire_cache`], seul appelant à qui
/// la nature de l'échec importe : un fichier **absent** est définitif, alors
/// qu'un accès refusé ou interrompu ne l'est pas et mérite un nouvel essai.
/// Confondre les deux ferait attendre le premier lancement d'une session, quand
/// le cache n'existe pas encore.
pub(crate) fn lire_texte_resultat(chemin: &Path) -> std::io::Result<String> {
    let octets = std::fs::read(chemin)?;
    let texte = String::from_utf8_lossy(&octets).into_owned();
    Ok(texte.strip_prefix('\u{feff}').unwrap_or(&texte).to_string())
}

/// Écrit un fichier par temporaire renommé, et dit si l'écriture a abouti.
///
/// Plusieurs sessions Claude Code partagent les fichiers d'état, et un
/// déplacement ne laisse jamais lire un fichier à moitié écrit — `fs::rename` a
/// sous Windows la sémantique de remplacement de `Move-Item -Force`. Le dossier
/// parent est créé au besoin, et le temporaire est retiré si l'une des deux
/// étapes échoue.
///
/// Le nom du temporaire conserve l'extension d'origine avant d'y ajouter le PID
/// (`statusline-cache.json.4312.tmp`) : c'est l'écriture qu'avait le cache
/// avant que cette fonction ne soit extraite, et le harnais de non-régression
/// compare l'état du dossier.
///
/// Toute erreur est absorbée : un fichier d'état qu'on n'a pas pu écrire ne
/// doit jamais coûter la ligne de statut.
pub(crate) fn ecrire_atomique(chemin: &Path, contenu: &[u8]) -> bool {
    if let Some(parent) = chemin.parent() {
        if !parent.exists() && std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }

    let extension = chemin
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    let temporaire = chemin.with_extension(format!("{}.{}.tmp", extension, std::process::id()));

    if std::fs::write(&temporaire, contenu).is_err() {
        let _ = std::fs::remove_file(&temporaire);
        return false;
    }
    if std::fs::rename(&temporaire, chemin).is_err() {
        let _ = std::fs::remove_file(&temporaire);
        return false;
    }
    true
}
