//! Conversions défensives — section 3 du script d'origine.
//!
//! Lecture du payload, et rien d'autre : chaque fonction rend `None` plutôt que
//! d'échouer, parce qu'une valeur inattendue ne doit coûter que le segment
//! qu'elle alimente, jamais la ligne entière. Plusieurs reproduisent
//! délibérément une véracité ou un arrondi de PowerShell — c'est là que se joue
//! l'équivalence avec l'oracle, et le harnais compare les deux sur 116 cas.

use serde_json::Value;

use crate::temps::jours_depuis_epoque;

/// Accède à un champ d'un objet JSON, ou rend `Value::Null`.
///
/// Reproduit l'accès de PowerShell, qui rend `$null` sur un membre absent comme
/// sur une valeur qui n'est pas un objet, sans jamais échouer.
pub(crate) fn champ<'a>(valeur: &'a Value, nom: &str) -> &'a Value {
    match valeur {
        Value::Object(m) => m.get(nom).unwrap_or(&Value::Null),
        _ => &Value::Null,
    }
}

/// Véracité d'une valeur au sens de PowerShell, telle que l'évalue `if ($x)`.
///
/// Les règles ne sont pas celles de Rust et gouvernent plusieurs branches du
/// script d'origine : une chaîne « 0 » est vraie parce qu'elle n'est pas vide,
/// un nombre 0 est faux, un tableau d'un seul élément vaut la véracité de cet
/// élément.
pub(crate) fn est_vrai(valeur: &Value) -> bool {
    match valeur {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|v| v != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => match a.len() {
            0 => false,
            1 => est_vrai(&a[0]),
            _ => true,
        },
        Value::Object(_) => true,
    }
}

/// Rend la forme texte d'une valeur scalaire, ou `None` si elle n'en a pas.
pub(crate) fn en_texte(valeur: &Value) -> Option<String> {
    match valeur {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "True".into() } else { "False".into() }),
        _ => None,
    }
}

/// Convertit une valeur quelconque en double, ou `None` si elle n'a pas de sens
/// numérique. Les valeurs non finies sont écartées : elles ne se formatent pas.
///
/// Reproduit `[System.Convert]::ToDouble` en culture invariante, plus permissif
/// que `str::parse` : les espaces encadrants sont tolérés, et un booléen vaut 1
/// ou 0.
pub(crate) fn convertir_nombre(valeur: &Value) -> Option<f64> {
    let nombre = match valeur {
        Value::Null => return None,
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().parse::<f64>().ok()?,
        _ => return None,
    };

    if nombre.is_nan() || nombre.is_infinite() {
        return None;
    }
    Some(nombre)
}

/// Arrondit comme le fait PowerShell : à l'entier pair le plus proche en cas
/// d'égalité (« arrondi du banquier »), là où `f64::round` s'écarte de zéro.
///
/// Le cas n'est pas théorique : `rate_limits.<fenêtre>.used_percentage` vaut
/// `utilization × 100` côté Claude Code, donc un `.5` exact est atteignable.
pub(crate) fn arrondir(nombre: f64) -> f64 {
    nombre.round_ties_even()
}

/// Convertit un pourcentage du payload en entier arrondi, ramené entre 0 et
/// 100, ou `None` si la valeur n'est pas exploitable.
///
/// Le débordement rend `None` là où PowerShell échouerait sur la conversion en
/// `[int]` : le script d'origine n'entoure pas ce cast, mais laisser échouer
/// irait contre l'invariant « rien n'est jamais fatal », et aucun pourcentage
/// réel n'approche la borne.
///
/// **Ramené dans les bornes depuis le 04/09/2026.** Un payload à 150 affichait
/// « 150% » et faisait projeter une heure d'épuisement dans le passé ; un
/// payload à −5 affichait « -5% ». Aucun compteur réel ne sort de 0 à 100 —
/// Claude Code calcule `utilization × 100` —, et une valeur qui en sort n'est
/// pas une mesure à afficher telle quelle. L'oracle applique la même borne, au
/// même endroit.
pub(crate) fn convertir_pourcent(valeur: &Value) -> Option<i64> {
    let nombre = arrondir(convertir_nombre(valeur)?);
    if nombre < i32::MIN as f64 || nombre > i32::MAX as f64 {
        return None;
    }
    Some((nombre as i64).clamp(0, 100))
}

/// Bornes de `DateTimeOffset`, en secondes Unix : 0001-01-01 et 9999-12-31.
/// Au-delà, `FromUnixTimeSeconds` échoue et le segment se retire.
const INSTANT_MIN: i64 = -62_135_596_800;
const INSTANT_MAX: i64 = 253_402_300_799;

/// Décalage horaire maximal admis dans une date ISO, en heures et dans les
/// deux sens : deux chiffres, comme en accepte l'oracle.
///
/// Ce n'est pas la borne du sens — aucun fuseau ne dépasse ±14:00, et
/// `DateTimeOffset.TryParse` s'y arrête — mais celle de l'oracle tel qu'il
/// reçoit la valeur : `ConvertFrom-Json` convertit lui-même les dates ISO du
/// payload en `[datetime]`, et son analyseur lit « +99:00 » sans broncher
/// quand il refuse « +9999999999999999:00 » — relevé sur le poste le
/// 04/09/2026. Ce que la borne ferme, c'est le débordement : un nombre
/// d'heures à seize chiffres multiplié par 3600 sortait du `i64` en silence.
const DECALAGE_MAX_HEURES: i64 = 99;

/// Même règle pour les minutes du décalage : deux chiffres, sans borne de sens
/// — `ConvertFrom-Json` lit « +05:60 » comme six heures, relevé le même jour.
const DECALAGE_MAX_MINUTES: i64 = 99;

/// Convertit un horodatage du payload en secondes Unix, ou `None` si la valeur
/// n'est pas exploitable.
///
/// `resets_at` est documenté en secondes Unix, et c'est bien ce qu'envoie
/// Claude Code. Deux autres écritures sont acceptées sans rien coûter, parce
/// qu'un format inattendu faisait auparavant disparaître la ligne entière
/// plutôt que le seul segment : l'ISO 8601 sous forme de chaîne, et les
/// millisecondes.
///
/// La chaîne datée passe avant la voie numérique. L'ordre importe peu en
/// pratique — vérifié le 21/08/2026, `DateTimeOffset.TryParse("20260802")`
/// rend `False` et une telle chaîne retombe donc sur les secondes — mais il
/// reproduit celui du script d'origine.
pub(crate) fn convertir_instant(valeur: &Value) -> Option<i64> {
    if let Value::String(s) = valeur {
        if let Some(instant) = analyser_iso(s) {
            return Some(instant);
        }
    }

    let nombre = convertir_nombre(valeur)?;

    // `[long]` arrondit en PowerShell, il ne tronque pas.
    let mut secondes = arrondir(nombre);

    // 1e11 secondes tombent en l'an 5138 : au-delà, l'unité est la milliseconde.
    if secondes.abs() >= 100_000_000_000.0 {
        secondes = arrondir(secondes / 1000.0);
    }

    if secondes < INSTANT_MIN as f64 || secondes > INSTANT_MAX as f64 {
        return None;
    }
    Some(secondes as i64)
}

/// Analyse une date ISO 8601 et rend son instant en secondes Unix.
///
/// Couvre ce que produit `DateTimeOffset.ToString("o")` — date, heure
/// facultative, fraction facultative, décalage `Z`, `±HH:mm` ou `±HHmm` — ainsi
/// que la date seule. Un décalage absent vaut UTC, comme le fait
/// `DateTimeStyles.AssumeUniversal` côté .NET.
///
/// Volontairement plus étroit que `TryParse`, qui accepte quantité de formats
/// localisés : le contrat d'entrée n'en produit aucun, et un parseur permissif
/// risquerait surtout d'avaler des chaînes que le script d'origine refuse.
///
/// **Tout ce qui n'est pas ASCII est rejeté d'entrée — 04/09/2026.** ISO 8601
/// ne s'écrit qu'en ASCII, et c'est cette garde qui rend sûres les coupes par
/// octets qui suivent : sur une chaîne ASCII, tout octet est une frontière de
/// caractère. Sans elle, un décalage horaire de quatre octets non ASCII faisait
/// paniquer la coupe `[0..2]`, et la ligne entière se réduisait au nom du
/// modèle au lieu de perdre le seul segment — constaté sur un payload forgé.
/// Le décalage est en outre borné à deux chiffres d'heures, comme le fait le
/// convertisseur JSON de l'oracle — voir [`DECALAGE_MAX_HEURES`] —, ce qui
/// ferme au passage un débordement silencieux du produit par 3600 sur un
/// nombre d'heures démesuré.
fn analyser_iso(texte: &str) -> Option<i64> {
    let t = texte.trim();
    if !t.is_ascii() || t.len() < 10 {
        return None;
    }

    let octets = t.as_bytes();
    if octets[4] != b'-' || octets[7] != b'-' {
        return None;
    }

    let annee: i64 = t.get(0..4)?.parse().ok()?;
    let mois: i64 = t.get(5..7)?.parse().ok()?;
    let jour: i64 = t.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&mois) || !(1..=31).contains(&jour) {
        return None;
    }

    let mut heure = 0i64;
    let mut minute = 0i64;
    let mut seconde = 0i64;
    let mut decalage = 0i64;

    let reste = &t[10..];
    if !reste.is_empty() {
        let premier = reste.as_bytes()[0];
        if premier != b'T' && premier != b't' && premier != b' ' {
            return None;
        }
        let mut heure_texte = &reste[1..];

        // Décalage horaire éventuel, retiré par la fin.
        if let Some(pos) = heure_texte.rfind(['+', '-']) {
            let signe = if heure_texte.as_bytes()[pos] == b'+' {
                1
            } else {
                -1
            };
            let brut = &heure_texte[pos + 1..];
            let (hh, mm) = if let Some((h, m)) = brut.split_once(':') {
                (h, m)
            } else if brut.len() == 4 {
                (&brut[0..2], &brut[2..4])
            } else {
                (brut, "0")
            };
            let hh: i64 = hh.parse().ok()?;
            let mm: i64 = mm.parse().ok()?;
            if !(0..=DECALAGE_MAX_HEURES).contains(&hh) || !(0..=DECALAGE_MAX_MINUTES).contains(&mm)
            {
                return None;
            }
            decalage = signe * (hh * 3600 + mm * 60);
            heure_texte = &heure_texte[..pos];
        } else if heure_texte.ends_with('Z') || heure_texte.ends_with('z') {
            heure_texte = &heure_texte[..heure_texte.len() - 1];
        }

        // Fraction de seconde : lue puis ignorée, l'affichage étant à la minute.
        if let Some((entier, _)) = heure_texte.split_once('.') {
            heure_texte = entier;
        }

        let mut morceaux = heure_texte.split(':');
        heure = morceaux.next()?.parse().ok()?;
        minute = morceaux.next().unwrap_or("0").parse().ok()?;
        seconde = morceaux.next().unwrap_or("0").parse().ok()?;
        if morceaux.next().is_some() {
            return None;
        }
        if !(0..=23).contains(&heure) || !(0..=59).contains(&minute) || !(0..=60).contains(&seconde)
        {
            return None;
        }
    }

    let jours = jours_depuis_epoque(annee, mois, jour);
    let instant = jours * 86_400 + heure * 3600 + minute * 60 + seconde - decalage;
    if !(INSTANT_MIN..=INSTANT_MAX).contains(&instant) {
        return None;
    }
    Some(instant)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn arrondi_au_pair_comme_powershell() {
        assert_eq!(arrondir(2.5), 2.0);
        assert_eq!(arrondir(3.5), 4.0);
        assert_eq!(arrondir(0.5), 0.0);
        assert_eq!(arrondir(-0.5), 0.0);
        assert_eq!(arrondir(29.4), 29.0);
        assert_eq!(arrondir(33.6), 34.0);
    }

    #[test]
    fn conversion_permissive_des_nombres() {
        assert_eq!(convertir_nombre(&json!("29.4")), Some(29.4));
        assert_eq!(convertir_nombre(&json!(" 29 ")), Some(29.0));
        assert_eq!(convertir_nombre(&json!("1e3")), Some(1000.0));
        assert_eq!(convertir_nombre(&json!(true)), Some(1.0));
        assert_eq!(convertir_nombre(&json!("beaucoup")), None);
        assert_eq!(convertir_nombre(&json!(null)), None);
        assert_eq!(convertir_nombre(&json!([1, 2])), None);
    }

    /// Le pourcentage est arrondi comme avant, puis ramené entre 0 et 100 ;
    /// une valeur hors du `[int]` de PowerShell reste un rejet, pas un
    /// plafond.
    #[test]
    fn pourcentage_ramene_entre_zero_et_cent() {
        assert_eq!(convertir_pourcent(&json!(150)), Some(100));
        assert_eq!(convertir_pourcent(&json!(-5)), Some(0));
        assert_eq!(convertir_pourcent(&json!(100.4)), Some(100));
        assert_eq!(convertir_pourcent(&json!(99.6)), Some(100));
        assert_eq!(convertir_pourcent(&json!("29.4")), Some(29));
        assert_eq!(convertir_pourcent(&json!(0)), Some(0));
        assert_eq!(convertir_pourcent(&json!(3_000_000_000_i64)), None);
        assert_eq!(convertir_pourcent(&json!(-3_000_000_000_i64)), None);
        assert_eq!(convertir_pourcent(&json!("beaucoup")), None);
    }

    #[test]
    fn instants_dans_leurs_trois_ecritures() {
        assert_eq!(
            convertir_instant(&json!(1_786_303_800_i64)),
            Some(1_786_303_800)
        );
        // Millisecondes : au-delà de 1e11, l'unité bascule.
        assert_eq!(
            convertir_instant(&json!(1_786_303_800_000_i64)),
            Some(1_786_303_800)
        );
        assert_eq!(
            convertir_instant(&json!("2026-08-21T15:04:21.7885401+00:00")),
            Some(1_787_324_661)
        );
        assert_eq!(
            convertir_instant(&json!("2026-08-21T17:04:21+02:00")),
            Some(1_787_324_661)
        );
        assert_eq!(convertir_instant(&json!("jamais")), None);
        // Hors de la plage représentable, malgré le repli sur les
        // millisecondes.
        assert_eq!(convertir_instant(&json!(999_999_999_999_999_i64)), None);
    }

    /// Les écritures du décalage horaire qu'accepte l'oracle, toutes ramenées
    /// au même instant : 15:04:21 UTC le 21/08/2026. Le « +20:00 » n'existe
    /// dans aucun fuseau, mais `ConvertFrom-Json` le lit, et le portage doit
    /// donc le lire aussi.
    #[test]
    fn decalages_horaires_dans_leurs_ecritures() {
        for texte in [
            "2026-08-21T15:04:21Z",
            "2026-08-21T17:04:21+02:00",
            "2026-08-21T20:34:21+0530",
            "2026-08-21T20:04:21+05",
            "2026-08-21T10:04:21-05:00",
            "2026-08-22T05:04:21+14:00",
            "2026-08-22T11:04:21+20:00",
            // Soixante minutes de décalage : six heures, comme pour l'oracle.
            "2026-08-21T21:04:21+05:60",
        ] {
            assert_eq!(
                convertir_instant(&json!(texte)),
                Some(1_787_324_661),
                "{texte}"
            );
        }
    }

    /// Chaînes adverses : aucune ne doit passer, et surtout aucune ne doit
    /// paniquer — une panique ici coûtait la ligne entière, pas le segment.
    #[test]
    fn analyser_iso_rejette_le_bruit_sans_paniquer() {
        for texte in [
            // Le payload forgé du 04/09/2026 : quatre octets non ASCII après
            // le signe, que la coupe `[0..2]` tranchait au milieu d'un
            // caractère.
            "2026-09-05T10:00:00+aéb",
            "２０２６-09-05",
            "2026-09-05Tééé",
            "2026-09-05T10:00:00.é+02:00",
            // Décalages au-delà de deux chiffres, dont un qui faisait déborder
            // le produit par 3600 en silence.
            "2026-09-05T10:00:00+9999999999999999:00",
            "2026-09-05T10:00:00+100:00",
            "2026-09-05T10:00:00+05:100",
            "2026-09-05T10:00:00+",
            "2026-09-05T10:00:00+:",
            "2026-09-05T",
            "2026-13-05",
            "2026-09-05T24:00:00",
            "2026-09-05T10:00:00:00",
        ] {
            assert_eq!(analyser_iso(texte), None, "{texte}");
        }
    }

    #[test]
    fn veracite_a_la_powershell() {
        assert!(est_vrai(&json!("0")));
        assert!(est_vrai(&json!("   ")));
        assert!(!est_vrai(&json!("")));
        assert!(!est_vrai(&json!(0)));
        assert!(!est_vrai(&json!(null)));
        assert!(!est_vrai(&json!([])));
        assert!(est_vrai(&json!(["a"])));
        assert!(est_vrai(&json!([1, 2])));
    }
}
