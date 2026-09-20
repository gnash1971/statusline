//! Temps : calendrier grégorien, heure locale du poste, mise en forme d'un
//! instant.
//!
//! Ce module n'a pas d'équivalent dans le script d'origine : PowerShell
//! disposait de `DateTimeOffset`, qui faisait à lui seul l'arithmétique
//! calendaire, la conversion de fuseau et le formatage. Il fallait donc les
//! réécrire, et les tenir à part — ce sont les seules fonctions du programme
//! dont la justesse ne se lit pas dans le code mais se vérifie par un aller et
//! retour.
//!
//! [`formater_instant`] les accompagne, venue de la section 6 : elle ne produit
//! pas de segment, elle habille un instant, et les deux affichages datés de la
//! ligne — remise à zéro et épuisement — passent par elle.

/// Nombre de jours entre le 1er janvier 1970 et une date civile.
///
/// Algorithme de Howard Hinnant, valable sur tout le calendrier grégorien
/// proleptique. Écrit ici plutôt qu'emprunté à `chrono` : c'est la seule
/// arithmétique calendaire dont ce programme ait besoin, et elle évite une
/// dépendance de plus.
pub(crate) fn jours_depuis_epoque(annee: i64, mois: i64, jour: i64) -> i64 {
    let a = if mois <= 2 { annee - 1 } else { annee };
    let ere = if a >= 0 { a } else { a - 399 } / 400;
    let annee_ere = a - ere * 400;
    let jour_annee = (153 * (if mois > 2 { mois - 3 } else { mois + 9 }) + 2) / 5 + jour - 1;
    let jour_ere = annee_ere * 365 + annee_ere / 4 - annee_ere / 100 + jour_annee;
    ere * 146_097 + jour_ere - 719_468
}

/// Date civile correspondant à un nombre de jours depuis l'époque Unix.
fn date_civile(jours: i64) -> (i64, i64, i64) {
    let z = jours + 719_468;
    let ere = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let jour_ere = z - ere * 146_097;
    let annee_ere = (jour_ere - jour_ere / 1460 + jour_ere / 36_524 - jour_ere / 146_096) / 365;
    let annee = annee_ere + ere * 400;
    let jour_annee = jour_ere - (365 * annee_ere + annee_ere / 4 - annee_ere / 100);
    let mp = (5 * jour_annee + 2) / 153;
    let jour = jour_annee - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    (if mois <= 2 { annee + 1 } else { annee }, mois, jour)
}

/// Instant décomposé en heure locale du poste.
struct HeureLocale {
    annee: i64,
    mois: i64,
    jour: i64,
    heure: i64,
    minute: i64,
}

/// Convertit un instant Unix en heure locale du poste.
///
/// Passe par `SystemTimeToTzSpecificLocalTime`, qui applique les règles du
/// fuseau courant, heure d'été comprise — c'est ce que fait
/// `DateTimeOffset.ToLocalTime()` côté .NET.
fn vers_heure_locale(instant: i64) -> Option<HeureLocale> {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::Time::SystemTimeToTzSpecificLocalTime;

    let jours = instant.div_euclid(86_400);
    let reste = instant.rem_euclid(86_400);
    let (annee, mois, jour) = date_civile(jours);

    // Hors de la plage acceptée par l'API Win32 (1601-30827).
    if !(1601..=30827).contains(&annee) {
        return None;
    }

    let utc = SYSTEMTIME {
        wYear: annee as u16,
        wMonth: mois as u16,
        wDayOfWeek: 0,
        wDay: jour as u16,
        wHour: (reste / 3600) as u16,
        wMinute: ((reste % 3600) / 60) as u16,
        wSecond: (reste % 60) as u16,
        wMilliseconds: 0,
    };

    let mut local = SYSTEMTIME {
        wYear: 0,
        wMonth: 0,
        wDayOfWeek: 0,
        wDay: 0,
        wHour: 0,
        wMinute: 0,
        wSecond: 0,
        wMilliseconds: 0,
    };

    // SAFETY : le fuseau nul désigne celui du poste d'après le contrat de
    // l'API, et les deux structures sont des locales valides, l'une lue et
    // l'autre écrite.
    let ok = unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) };
    if ok == 0 {
        return None;
    }

    Some(HeureLocale {
        annee: local.wYear as i64,
        mois: local.wMonth as i64,
        jour: local.wDay as i64,
        heure: local.wHour as i64,
        minute: local.wMinute as i64,
    })
}

/// Date du jour, en heure locale.
fn aujourdhui() -> (i64, i64, i64) {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    let mut st = SYSTEMTIME {
        wYear: 0,
        wMonth: 0,
        wDayOfWeek: 0,
        wDay: 0,
        wHour: 0,
        wMinute: 0,
        wSecond: 0,
        wMilliseconds: 0,
    };
    // SAFETY : `st` est une structure locale valide que l'API remplit ; l'appel
    // n'échoue jamais d'après son contrat.
    unsafe { GetLocalTime(&mut st) };
    (st.wYear as i64, st.wMonth as i64, st.wDay as i64)
}

/// Instant présent, en secondes Unix.
pub(crate) fn maintenant() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Met en forme un instant : l'heure seule s'il tombe aujourd'hui, la date
/// devant sinon. Les deux instants affichés — remise à zéro et épuisement —
/// suivent la même règle.
pub(crate) fn formater_instant(instant: i64) -> Option<String> {
    let local = vers_heure_locale(instant)?;
    let (a, m, j) = aujourdhui();

    if (local.annee, local.mois, local.jour) == (a, m, j) {
        Some(format!("{:02}:{:02}", local.heure, local.minute))
    } else {
        Some(format!(
            "{:02}/{:02} {:02}:{:02}",
            local.jour, local.mois, local.heure, local.minute
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendrier_aller_retour() {
        for jours in [-25_000_i64, 0, 1, 20_320, 100_000] {
            let (a, m, j) = date_civile(jours);
            assert_eq!(jours_depuis_epoque(a, m, j), jours);
        }
    }
}
