//! Largeur de la console : ce que le terminal peut afficher sur un rang.
//!
//! Le payload ne dit rien de la largeur du terminal, et la ligne de statut
//! n'écrit pas sur une console — Claude Code lui donne un tube. Mais le
//! processus reste **attaché** à la console de Claude Code : il en hérite au
//! lancement, comme tout processus console lancé sans `DETACHED_PROCESS` ni
//! `CREATE_NO_WINDOW`, et cette console-là est celle du terminal, à travers le
//! pseudo-terminal que Windows lui donne. Ouvrir `CONOUT$` la retrouve, quel
//! que soit l'état des descripteurs standard.
//!
//! Relevé le 19/09/2026, au soir de la capsule : Claude Code n'a pas coupé une
//! ligne plus large que son terminal, il l'a **repliée** sur le rang suivant,
//! caps compris — la forme se casse. Connaître la largeur est ce qui permet de
//! replier proprement, à une frontière de compartiment. Sans console — un autre
//! lanceur, un harnais qui redirige tout —, la sonde rend `None` et la ligne
//! garde sa disposition fixe.

use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Console::{GetConsoleScreenBufferInfo, CONSOLE_SCREEN_BUFFER_INFO};

/// Rend la largeur de la fenêtre de console, en cellules, ou `None` si le
/// processus n'a pas de console.
///
/// C'est la fenêtre visible qui est mesurée — `srWindow`, et non `dwSize`, le
/// tampon —, parce que c'est elle que le terminal replie. Sous Windows Terminal
/// les deux se confondent ; sous une console classique, le tampon peut être
/// plus large que la fenêtre, et c'est bien la fenêtre qui compte.
pub(crate) fn largeur_console() -> Option<u16> {
    // « CONOUT$ », en UTF-16 terminé par zéro.
    let nom: Vec<u16> = "CONOUT$\0".encode_utf16().collect();

    // SAFETY : appels Win32 sur des arguments valides ; le descripteur ouvert
    // est refermé sur tous les chemins, et la structure est initialisée à zéro
    // avant d'être remplie par l'API.
    unsafe {
        let console = CreateFileW(
            nom.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        if console == INVALID_HANDLE_VALUE {
            return None;
        }

        let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
        let reussi = GetConsoleScreenBufferInfo(console, &mut info) != 0;
        CloseHandle(console);
        if !reussi {
            return None;
        }

        let largeur = i32::from(info.srWindow.Right) - i32::from(info.srWindow.Left) + 1;
        u16::try_from(largeur).ok().filter(|l| *l > 0)
    }
}
