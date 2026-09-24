//! Fenêtres de limitation : pourcentage consommé, rythme, remise à zéro.
//!
//! Extrait de la section 6, dont ce module tient la moitié la plus délicate —
//! la seule à mesurer une pente, à écrire dans le cache et à annoncer quelque
//! chose qui n'est pas encore arrivé. Les segments qui ne font que mettre en
//! forme une valeur du payload vivent dans [`crate::segments`].
//!
//! Depuis le soir du 03/09/2026, une troisième fenêtre peut s'y joindre : celle
//! que `/usage` relève pour le modèle courant, lue dans « ~/.claude.json » par
//! [`crate::usage`]. Elle se dispute la place de la fenêtre hebdomadaire avec
//! celle du payload : c'est elle qui l'occupe, sauf quand la globale est en
//! alerte et plus remplie — voir [`retenir`].
//!
//! Et depuis le 05/09/2026, cette fenêtre-là peut s'afficher **estimée** : le
//! relevé ne bouge qu'à l'ouverture de `/usage`, mais la globale du payload est
//! fraîche à chaque réponse, et ce qu'elle a pris depuis le relevé donne la
//! mesure de ce que la fenêtre du modèle a pris aussi — voir
//! [`estimer_derive`] et le marqueur « ≈ » de [`formater_fenetre`].

use serde_json::{json, Value};

use crate::cache::{ecrire_cache, fenetre_memorisee, lire_cache, FenetreAMemoriser};
use crate::conversions::{
    arrondir, champ, convertir_instant, convertir_nombre, convertir_pourcent,
};
use crate::modele::{descripteur_effectif, facteur_famille, famille_modele};
use crate::reglages::{
    Fenetre, FENETRES, FENETRE_MODELE, MARQUEUR_ESTIMEE, MARQUEUR_MEMORISEE, PREFIXE_EPUISEMENT,
    PREFIXE_PROJECTION, RATIO_MAX_EXTRAPOLATION, SEPARATEUR_INTERNE, SPAN_MINIMUM_RYTHME,
    UNITE_POURCENT,
};
use crate::sortie::{
    attenuer, au_palier, formater_mesure, formater_mesure_jaugee, palier, Palier, Segment,
};
use crate::temps::{formater_instant, maintenant};
use crate::usage::{estimer_derive, fenetre_modele};

/// Ancrage de mesure d'une fenêtre : instant et pourcentage de la première
/// observation faite sur cette fenêtre-là, et — pour les fenêtres qui suivent
/// le modèle — la famille sous laquelle cette observation a été faite.
struct Ancrage {
    observe_a: i64,
    observe_pourcent: Option<i64>,
    observe_modele: Option<String>,
}

/// Segment d'une fenêtre, avec le pourcentage qui l'a composé et les paliers
/// qu'il a franchis : c'est sur eux que deux fenêtres candidates à la même
/// place sur la ligne se départagent — voir [`retenir`] — et c'est le pire
/// d'entre eux qui teinte le compartiment des mesures.
struct Candidat {
    segment: String,
    pourcent: i64,
    /// Palier du pourcentage affiché, aux seuils du descripteur effectif :
    /// celui qui départage deux candidates et qui appelle l'heure de remise à
    /// zéro.
    palier: Palier,
    /// Pire palier de tout le segment, rythme compris : une projection en
    /// alerte ou un épuisement annoncé teintent le compartiment sans changer
    /// l'arbitrage des places — voir [`crate::sortie::teinte_du_palier`].
    pire: Palier,
}

impl Candidat {
    /// Pourcentage au seuil d'alerte du descripteur effectif, ou au-delà.
    fn en_alerte(&self) -> bool {
        self.palier >= Palier::Alerte
    }
}

/// Rend la famille mémorisée avec un ancrage, ou `None` si le cache n'en porte
/// pas — cache antérieur au 03/09/2026, ou fenêtre qui ne suit pas le modèle.
fn famille_memorisee(memorisee: &Value) -> Option<&str> {
    match champ(memorisee, "observe_modele") {
        Value::String(s) if !s.trim().is_empty() => Some(s.as_str()),
        _ => None,
    }
}

/// Détermine l'ancrage d'une fenêtre fraîche.
///
/// Une fenêtre est identifiée par son heure de remise à zéro. Tant qu'elle ne
/// bouge pas, l'ancrage mémorisé est conservé et la durée d'observation
/// s'allonge — y compris d'une session à l'autre, ce qui rend la pente
/// hebdomadaire exploitable là où une seule session ne mesurerait rien.
///
/// Quatre situations réancrent sur l'instant présent, parce que l'ancrage
/// mémorisé ne décrit plus la même chose :
///   - remise à zéro différente : c'est une autre fenêtre ;
///   - pourcentage en recul : le compteur ne redescend pas au fil d'une
///     fenêtre, une correction côté serveur invaliderait la pente ;
///   - ancrage postérieur à maintenant : l'horloge du poste a reculé entre deux
///     sessions, et la durée d'observation serait négative ;
///   - **famille de modèle différente** — 03/09/2026 —, pour les seules
///     fenêtres qui suivent le modèle : une pente mesurée sous Opus ne dit rien
///     de ce que Fable va consommer, à deux fois le prix du jeton. Le segment
///     se tait alors le temps de réobserver, ce qui est le comportement voulu —
///     une projection fausse coûte plus cher qu'un segment absent.
///
/// Le réancrage par famille demande les **deux** familles : la courante et la
/// mémorisée. Un modèle hors table, ou un cache antérieur au champ, n'est pas un
/// changement mais une inconnue, et l'ancrage tient. Un ancrage conservé garde
/// sa famille ; s'il n'en avait pas, il prend celle du jour, faute de mieux.
fn calculer_ancrage(
    fraiche: &Value,
    memorisee: Option<&Value>,
    instant: i64,
    famille: Option<&str>,
    ancrage_selon_modele: bool,
) -> Ancrage {
    let pourcent = convertir_pourcent(champ(fraiche, "used_percentage"));

    let famille_neuve = if ancrage_selon_modele {
        famille.map(str::to_string)
    } else {
        None
    };
    let neuf = Ancrage {
        observe_a: instant,
        observe_pourcent: pourcent,
        observe_modele: famille_neuve.clone(),
    };

    let Some(memorisee) = memorisee else {
        return neuf;
    };
    let Some(pourcent) = pourcent else {
        return neuf;
    };

    let reset = convertir_instant(champ(fraiche, "resets_at"));
    let reset_memorise = convertir_instant(champ(memorisee, "resets_at"));
    match (reset, reset_memorise) {
        (Some(a), Some(b)) if a == b => {}
        _ => return neuf,
    }

    let ancre = convertir_nombre(champ(memorisee, "observe_a")).map(arrondir);
    let ancre_pourcent = convertir_pourcent(champ(memorisee, "observe_pourcent"));
    let (Some(ancre), Some(ancre_pourcent)) = (ancre, ancre_pourcent) else {
        return neuf;
    };

    if pourcent < ancre_pourcent || ancre > instant as f64 {
        return neuf;
    }

    let conservee = famille_memorisee(memorisee);
    if ancrage_selon_modele {
        if let (Some(courante), Some(memorisee)) = (famille, conservee) {
            if courante != memorisee {
                return neuf;
            }
        }
    }

    Ancrage {
        observe_a: ancre as i64,
        observe_pourcent: Some(ancre_pourcent),
        observe_modele: if ancrage_selon_modele {
            conservee.map(str::to_string).or(famille_neuve)
        } else {
            None
        },
    }
}

/// Met en forme le rythme de consommation d'une fenêtre, ou `None` quand il n'y
/// a rien d'honnête à en dire.
///
/// La pente est mesurée entre l'ancrage et maintenant, jamais déduite d'une
/// durée de fenêtre supposée : rien dans le payload ne dit qu'une fenêtre
/// « five_hour » dure cinq heures pleines, et l'hypothèse se paierait sur
/// toutes les valeurs affichées.
///
/// Deux affichages, exclusifs l'un de l'autre par construction — la projection
/// atteint 100 % exactement quand l'épuisement précède la remise à zéro :
///   - « → 61% », consommation prévue à la fin de la fenêtre, soumise aux mêmes
///     paliers de coloration que le pourcentage courant ;
///   - « épuisé 14:10 », heure prévue du plafond, toujours au palier critique :
///     c'est le seul cas où la ligne annonce quelque chose qui n'est pas encore
///     arrivé.
///
/// Se taire est ici le comportement utile, pas un pis-aller : une projection
/// fausse mais affirmée coûte plus cher qu'un segment absent, puisqu'elle porte
/// la couleur du palier critique et finit par déprécier celle-ci partout
/// ailleurs sur la ligne.
fn formater_rythme(
    fenetre: &Value,
    ancrage: &Ancrage,
    descripteur: &Fenetre,
    instant: i64,
) -> Option<(String, Palier)> {
    let ancre_pourcent = ancrage.observe_pourcent?;

    let pourcent = convertir_pourcent(champ(fenetre, "used_percentage"))?;
    let reset = convertir_instant(champ(fenetre, "resets_at"))?;

    let ecoule = instant - ancrage.observe_a;
    if ecoule < SPAN_MINIMUM_RYTHME * 60 {
        return None;
    }

    let consomme = pourcent - ancre_pourcent;
    if consomme <= 0 {
        return None;
    }

    let restant = reset - instant;
    if restant <= 0 {
        return None;
    }

    // La durée à projeter doit rester du même ordre que la durée observée.
    if restant > ecoule * RATIO_MAX_EXTRAPOLATION {
        return None;
    }

    // Pente en points de pourcentage par seconde.
    let pente = consomme as f64 / ecoule as f64;
    let projection = pourcent as f64 + pente * restant as f64;

    if projection >= 100.0 {
        // Borné par `restant`, puisqu'on n'entre ici que si la projection
        // atteint 100 % avant la remise à zéro.
        let secondes = ((100 - pourcent) as f64 / pente) as i64;
        let epuisement = formater_instant(instant + secondes)?;

        // Seul fragment intégralement coloré, préfixe compris : il n'annonce
        // pas une mesure mais une échéance, et son libellé fait partie de
        // l'avertissement plutôt que du chrome. Au palier critique quel que
        // soit le pourcentage courant : dire que le plafond tombera avant la
        // remise à zéro, c'est déjà annoncer un 100 % — et le compartiment
        // des mesures en prend le rouge sombre.
        return Some((
            au_palier(
                &format!("{} {}", PREFIXE_EPUISEMENT, epuisement),
                Palier::Critique,
            ),
            Palier::Critique,
        ));
    }

    let arrondie = arrondir(projection) as i64;
    if arrondie <= pourcent {
        return None;
    }

    Some(formater_mesure(
        PREFIXE_PROJECTION,
        &arrondie.to_string(),
        UNITE_POURCENT,
        arrondie,
        descripteur.seuil,
        descripteur.seuil_critique,
    ))
}

/// Met en forme une fenêtre de limitation : pourcentage consommé, rythme, puis
/// heure de remise à zéro. Rend aussi le pourcentage, pour départager deux
/// fenêtres candidates à la même place.
///
/// L'heure de remise à zéro est préférée à un décompte restant : elle ne dépend
/// pas de la fréquence de rafraîchissement de la ligne, et se planifie plus
/// directement. Elle n'est affichée que si le descripteur l'exige, ou si le
/// seuil d'alerte est franchi. Un pourcentage inexploitable retire donc la
/// fenêtre entière, heure comprise — sans le pourcentage, elle ne situe plus
/// rien.
///
/// `memorisee` marque le pourcentage d'un « ~ » : la valeur vient d'un cache et
/// non d'une mesure fraîche — celui de la ligne avant la première réponse de la
/// session, ou un relevé de `/usage` vieux de plus d'une heure — et ne doit pas
/// passer pour telle. `estimation`, quand elle existe, **remplace** le
/// pourcentage de la fenêtre et le marque d'un « ≈ » — 05/09/2026 : la fenêtre
/// du modèle avancée de ce que la globale a pris depuis le relevé, voir
/// [`estimer_derive`]. Elle l'emporte sur `memorisee` : estimée, la valeur
/// n'est plus ancienne. Dans les deux cas le rythme est tu — projeter à partir
/// d'une mesure déjà périmée, ou déjà estimée, empilerait deux approximations.
fn formater_fenetre(
    fenetre: &Value,
    descripteur: &Fenetre,
    ancrage: Option<&Ancrage>,
    instant: i64,
    memorisee: bool,
    estimation: Option<i64>,
) -> Option<Candidat> {
    if fenetre.is_null() {
        return None;
    }

    let mesure = convertir_pourcent(champ(fenetre, "used_percentage"))?;
    let (pourcent, prefixe, figee) = match estimation {
        Some(estime) => (estime, MARQUEUR_ESTIMEE, true),
        None if memorisee => (mesure, MARQUEUR_MEMORISEE, true),
        None => (mesure, "", false),
    };
    // Le rythme d'abord : son palier entre dans celui du segment, dont la
    // jauge fine doit connaître le fond avant de s'écrire — Q2 du chantier
    // « mesures », 24/09/2026. Il ne lit que la fenêtre et l'ancrage, rien de
    // ce que la mesure produit.
    let rythme = if figee {
        None
    } else {
        ancrage.and_then(|ancrage| formater_rythme(fenetre, ancrage, descripteur, instant))
    };
    let pire = rythme
        .as_ref()
        .map_or(Palier::Aucun, |(_, palier_rythme)| *palier_rythme)
        .max(palier(
            pourcent,
            descripteur.seuil,
            descripteur.seuil_critique,
        ));

    // Le marqueur reste collé à la valeur : il qualifie la mesure, pas le
    // compteur. La jauge, elle, précède les deux : elle annonce le remplissage
    // que le nombre chiffre, et vaut pour une valeur mémorisée ou estimée comme
    // pour une mesure fraîche — celle du cache est ancienne, pas fausse.
    let (mesure, palier) = formater_mesure_jaugee(
        descripteur.libelle,
        &format!("{}{}", prefixe, pourcent),
        UNITE_POURCENT,
        pourcent,
        descripteur.seuil,
        descripteur.seuil_critique,
        pire,
    );
    let mut morceaux = vec![mesure];
    let en_alerte = palier >= Palier::Alerte;

    if let Some((rythme, _)) = rythme {
        morceaux.push(rythme);
    }

    if descripteur.reset_toujours || en_alerte {
        if let Some(instant_reset) = convertir_instant(champ(fenetre, "resets_at")) {
            if let Some(affiche) = formater_instant(instant_reset) {
                // Atténuée : c'est un repère de planification, pas une valeur à
                // surveiller.
                //
                // Le mot « reset » qui la précédait est tombé le 25/08/2026. Une
                // heure qui suit un pourcentage, dans un segment de fenêtre, ne
                // peut désigner qu'une remise à zéro — et le mot coûtait six
                // colonnes par fenêtre, doublées dans le cas d'alerte, c'est-à-
                // dire là où la ligne est déjà à son plus long. L'heure
                // d'épuisement, elle, garde son libellé et sa couleur critique :
                // il y porte un avertissement, pas une étiquette.
                morceaux.push(attenuer(&affiche));
            }
        }
    }

    Some(Candidat {
        segment: morceaux.join(SEPARATEUR_INTERNE),
        pourcent,
        palier,
        pire,
    })
}

/// Ce qu'une mesure demande en plus de la fenêtre elle-même.
///
/// Regroupé le 04/09/2026 : [`mesurer`] prenait huit paramètres positionnels,
/// dont deux descripteurs du même type et trois valeurs qu'un appelant
/// pouvait intervertir sans que rien ne le signale. Les champs nommés disent
/// lequel est lequel.
struct Mesure<'a> {
    /// Règles d'ancrage et clé de cache.
    descripteur: &'a Fenetre,
    /// Seuils tels qu'ils s'appliquent sous le modèle courant — ne diffère du
    /// descripteur que pour la fenêtre hebdomadaire du payload.
    effectif: &'a Fenetre,
    /// Entrée du cache pour cette fenêtre, si elle tient encore.
    memorisee: Option<&'a Value>,
    /// Instant de la mesure : maintenant, ou celui du relevé de `/usage`.
    instant: i64,
    /// Famille du modèle de la session, pour l'ancrage.
    famille: Option<&'a str>,
    /// Affiche la valeur comme une valeur de cache, sans rythme : c'est le cas
    /// d'un relevé de `/usage` vieux de plus d'une heure, que l'on ancre et
    /// mémorise quand même, puisque la mesure est réelle.
    ancienne: bool,
    /// Pourcentage estimé par dérive — 05/09/2026 —, qui remplace à
    /// l'affichage celui de la fenêtre, marqué « ≈ » et sans rythme. Le cache
    /// n'en sait rien : il mémorise la fenêtre telle que relevée. Voir
    /// [`estimer_derive`].
    estimation: Option<i64>,
}

/// Mesure une fenêtre fraîche : pose ou conserve son ancrage, la remet au
/// cache, et la met en forme. Voir [`Mesure`] pour ce qui l'accompagne.
fn mesurer(
    fraiche: &Value,
    mesure: &Mesure<'_>,
    a_memoriser: &mut Vec<FenetreAMemoriser>,
) -> Option<Candidat> {
    let ancrage = calculer_ancrage(
        fraiche,
        mesure.memorisee,
        mesure.instant,
        mesure.famille,
        mesure.descripteur.ancrage_selon_modele,
    );

    a_memoriser.push(FenetreAMemoriser {
        cle: mesure.descripteur.cle,
        used_percentage: champ(fraiche, "used_percentage").clone(),
        resets_at: champ(fraiche, "resets_at").clone(),
        observe_a: Value::from(ancrage.observe_a),
        observe_pourcent: ancrage
            .observe_pourcent
            .map(Value::from)
            .unwrap_or(Value::Null),
        observe_modele: ancrage
            .observe_modele
            .clone()
            .map(Value::from)
            .unwrap_or(Value::Null),
    });

    formater_fenetre(
        fraiche,
        mesure.effectif,
        Some(&ancrage),
        mesure.instant,
        mesure.ancienne,
        mesure.estimation,
    )
}

/// Reprend une fenêtre du cache de la ligne, avant la première réponse de la
/// session : l'ancrage mémorisé est réécrit tel quel, famille comprise, faute de
/// nouvelle mesure à y verser, et la valeur s'affiche marquée « ~ ».
fn reprendre(
    memorisee: &Value,
    descripteur: &Fenetre,
    effectif: &Fenetre,
    instant: i64,
    a_memoriser: &mut Vec<FenetreAMemoriser>,
) -> Option<Candidat> {
    a_memoriser.push(FenetreAMemoriser {
        cle: descripteur.cle,
        used_percentage: champ(memorisee, "used_percentage").clone(),
        resets_at: champ(memorisee, "resets_at").clone(),
        observe_a: champ(memorisee, "observe_a").clone(),
        observe_pourcent: champ(memorisee, "observe_pourcent").clone(),
        observe_modele: champ(memorisee, "observe_modele").clone(),
    });

    formater_fenetre(memorisee, effectif, None, instant, true, None)
}

/// Retient, entre l'occupant d'une place et une nouvelle candidate, celle qui
/// décrit la session. La nouvelle est la fenêtre propre au modèle — celle que
/// `/usage` attribue à la session — et elle prend la place, **sauf** si
/// l'occupant, la fenêtre globale, est en alerte et strictement plus rempli :
/// c'est alors lui qui menace la session, et le cacher coûterait l'alerte. À
/// égalité, la fenêtre du modèle.
///
/// **La règle a changé le 04/09/2026.** La place allait jusque-là à la plus
/// remplie, sans condition, au motif que c'est elle qui bornera la session la
/// première. Le lendemain d'une remise à zéro hebdomadaire, cela donnait la
/// place à une fenêtre globale à 2 % contre une fenêtre Fable à 0 % : la ligne
/// ne disait plus rien du modèle qui tourne, alors que sa fenêtre se remplit
/// deux fois plus vite que la globale et la rattrape en une heure de travail.
/// Tant que la globale n'a rien à signaler, seule la fenêtre du modèle décrit
/// ce que la session consomme ; dès qu'elle alerte, la globale reprend la place
/// si elle est la plus remplie — le cas qui avait motivé l'ancienne règle, et
/// que la nouvelle couvre encore.
fn retenir(place: &mut Option<Candidat>, nouvelle: Option<Candidat>) {
    let Some(nouvelle) = nouvelle else {
        return;
    };
    let remplace = match place {
        Some(occupant) => !(occupant.en_alerte() && occupant.pourcent > nouvelle.pourcent),
        None => true,
    };
    if remplace {
        *place = Some(nouvelle);
    }
}

/// Produit les segments des fenêtres de limitation, et mémorise au passage les
/// valeurs fraîches pour les prochains lancements de session.
///
/// Le cache est relu une seule fois pour toutes les fenêtres, et réécrit une
/// seule fois : la ligne se rafraîchit souvent, et chaque fenêtre traitée
/// séparément doublerait les accès disque sans rien apporter.
///
/// Les fenêtres reprises du cache y sont remises telles quelles. C'est ce qui
/// préserve « seven_day » quand le payload ne porte que « five_hour », les deux
/// pouvant être absentes indépendamment l'une de l'autre.
///
/// **Le modèle de la session entre ici — 03/09/2026.** Sa famille donne le
/// facteur de consommation, le facteur donne à la fenêtre hebdomadaire du
/// payload son descripteur effectif — seuils abaissés sous Fable —, et la
/// famille elle-même va à l'ancrage, qui repart quand elle change. Une fenêtre
/// relue du cache est colorée aux seuils **du jour** : ils décrivent ce que la
/// session va consommer, pas ce qu'une autre a consommé.
///
/// **Et sa fenêtre propre, le soir même.** Quand `/usage` a relevé une fenêtre
/// hebdomadaire pour cette famille — voir [`crate::usage`] —, elle est mesurée
/// comme les autres, sous sa propre clé de cache et à l'instant de son relevé,
/// puis elle se dispute la place « 7j » avec la fenêtre du payload — voir
/// [`retenir`] pour l'arbitrage. Sur ce poste le 03/09/2026 au soir, 28 % pour
/// Fable contre 14 % pour tous les modèles — et la ligne dit 28, comme
/// `/usage` ; le lendemain, 0 % pour Fable contre 2 %, et la ligne dit 0, comme
/// `/usage` encore.
///
/// **Chaque fenêtre rend son palier depuis le 24/09/2026** — chantier
/// « mesures », piste A : le pire de ses valeurs, rythme compris, qui décide
/// de son fond et de son contour. Le pire des fenêtres ne se calcule plus ici :
/// aucune fenêtre ne teint plus ses voisines.
pub(crate) fn segment_fenetres(donnees: &Value, config: Option<&Value>) -> Vec<Segment> {
    // Un seul instant de référence pour tout le passage : la pente affichée et
    // l'ancrage écrit dans le cache doivent parler de la même seconde.
    let instant = maintenant();

    let famille = famille_modele(donnees);
    let facteur = facteur_famille(famille);

    let cache = lire_cache();
    let mut a_memoriser: Vec<FenetreAMemoriser> = Vec::new();

    // Une place par libellé, dans l'ordre des fenêtres du payload.
    let mut places: Vec<(&'static str, Option<Candidat>)> = Vec::new();

    for descripteur in FENETRES.iter() {
        let effectif = descripteur_effectif(descripteur, facteur);
        let fraiche = champ(champ(donnees, "rate_limits"), descripteur.cle);
        let memorisee = fenetre_memorisee(cache.as_ref(), descripteur.cle, instant);

        let candidat = if !fraiche.is_null() && !champ(fraiche, "used_percentage").is_null() {
            mesurer(
                fraiche,
                &Mesure {
                    descripteur,
                    effectif: &effectif,
                    memorisee: memorisee.as_ref(),
                    instant,
                    famille,
                    ancienne: false,
                    estimation: None,
                },
                &mut a_memoriser,
            )
        } else if let Some(memorisee) = memorisee {
            // Avant la première requête de la session, « rate_limits » n'est
            // pas renseigné : on retombe sur la dernière valeur connue.
            reprendre(
                &memorisee,
                descripteur,
                &effectif,
                instant,
                &mut a_memoriser,
            )
        } else {
            None
        };

        places.push((descripteur.libelle, candidat));
    }

    if let Some(releve) = fenetre_modele(config, famille, instant) {
        let descripteur = &FENETRE_MODELE;
        let ancienne = releve.ancienne(instant);
        let fraiche = json!({
            "used_percentage": releve.used_percentage,
            "resets_at": releve.resets_at,
        });
        let memorisee = fenetre_memorisee(cache.as_ref(), descripteur.cle, instant);

        // La fenêtre du payload qui partage la place — la globale de la
        // semaine — est fraîche à chaque réponse : ce qu'elle a pris depuis le
        // relevé, la fenêtre du modèle l'a pris aussi. Voir `estimer_derive`.
        let globale = FENETRES
            .iter()
            .find(|f| f.libelle == descripteur.libelle)
            .map_or(&Value::Null, |f| {
                champ(champ(donnees, "rate_limits"), f.cle)
            });
        let estimation = estimer_derive(&releve, globale);

        // Mesurée à l'instant de son relevé, et non maintenant : c'est à cet
        // instant-là que le pourcentage était vrai, et la pente comme la
        // projection se calculent dans ce repère.
        let candidat = mesurer(
            &fraiche,
            &Mesure {
                descripteur,
                effectif: descripteur,
                memorisee: memorisee.as_ref(),
                instant: releve.releve_a,
                famille,
                ancienne,
                estimation,
            },
            &mut a_memoriser,
        );

        match places
            .iter_mut()
            .find(|(libelle, _)| *libelle == descripteur.libelle)
        {
            Some((_, place)) => retenir(place, candidat),
            None => places.push((descripteur.libelle, candidat)),
        }
    }

    ecrire_cache(&a_memoriser);

    // Un segment par fenêtre, et non les fenêtres déjà jointes par « · » —
    // lot 2 du chantier : c'est le compartiment qui les joint, et c'est entre
    // elles que le repli en largeur peut couper. Chacun porte son pire palier ;
    // une fenêtre écartée par `retenir` ne compte pour rien, puisqu'elle ne
    // s'affiche pas.
    places
        .into_iter()
        .filter_map(|(_, candidat)| candidat)
        .filter(|candidat| !candidat.segment.is_empty())
        .map(|candidat| Segment {
            texte: candidat.segment,
            palier: candidat.pire,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Fenêtre fraîche à 20 %, remise à zéro dans quatre jours.
    fn fraiche(reset: i64) -> Value {
        json!({ "used_percentage": 20, "resets_at": reset })
    }

    /// Ancrage mémorisé deux jours plus tôt à 10 %, avec ou sans famille.
    fn memorisee(reset: i64, ancre: i64, famille: Option<&str>) -> Value {
        let mut entree = json!({
            "used_percentage": 15,
            "resets_at": reset,
            "observe_a": ancre,
            "observe_pourcent": 10
        });
        if let Some(f) = famille {
            entree["observe_modele"] = Value::from(f);
        }
        entree
    }

    const MAINTENANT: i64 = 1_788_400_000;
    const RESET: i64 = MAINTENANT + 4 * 86_400;
    const ANCRE: i64 = MAINTENANT - 2 * 86_400;

    #[test]
    fn un_changement_de_famille_reancre_la_fenetre_qui_suit_le_modele() {
        let cache = memorisee(RESET, ANCRE, Some("opus"));
        let ancrage = calculer_ancrage(
            &fraiche(RESET),
            Some(&cache),
            MAINTENANT,
            Some("fable"),
            true,
        );

        // Ancrage neuf, étiqueté de la famille du jour : la pente d'Opus ne
        // dit rien de Fable.
        assert_eq!(ancrage.observe_a, MAINTENANT);
        assert_eq!(ancrage.observe_pourcent, Some(20));
        assert_eq!(ancrage.observe_modele.as_deref(), Some("fable"));
    }

    #[test]
    fn la_meme_famille_conserve_l_ancrage() {
        let cache = memorisee(RESET, ANCRE, Some("fable"));
        let ancrage = calculer_ancrage(
            &fraiche(RESET),
            Some(&cache),
            MAINTENANT,
            Some("fable"),
            true,
        );

        assert_eq!(ancrage.observe_a, ANCRE);
        assert_eq!(ancrage.observe_pourcent, Some(10));
        assert_eq!(ancrage.observe_modele.as_deref(), Some("fable"));
    }

    #[test]
    fn une_famille_inconnue_n_est_pas_un_changement() {
        // Modèle hors table aujourd'hui : l'ancrage tient, et garde sa famille.
        let cache = memorisee(RESET, ANCRE, Some("opus"));
        let ancrage = calculer_ancrage(&fraiche(RESET), Some(&cache), MAINTENANT, None, true);
        assert_eq!(ancrage.observe_a, ANCRE);
        assert_eq!(ancrage.observe_modele.as_deref(), Some("opus"));

        // Cache antérieur au champ : l'ancrage tient, et prend la famille du
        // jour faute de mieux.
        let ancien = memorisee(RESET, ANCRE, None);
        let ancrage = calculer_ancrage(
            &fraiche(RESET),
            Some(&ancien),
            MAINTENANT,
            Some("fable"),
            true,
        );
        assert_eq!(ancrage.observe_a, ANCRE);
        assert_eq!(ancrage.observe_modele.as_deref(), Some("fable"));

        // Ni l'un ni l'autre : rien à comparer, rien à écrire.
        let ancrage = calculer_ancrage(&fraiche(RESET), Some(&ancien), MAINTENANT, None, true);
        assert_eq!(ancrage.observe_a, ANCRE);
        assert_eq!(ancrage.observe_modele, None);
    }

    #[test]
    fn une_fenetre_qui_ne_suit_pas_le_modele_ignore_la_famille() {
        // Même changement Opus → Fable : la fenêtre de 5 heures garde son
        // ancrage, et n'écrit jamais de famille — pas même celle du cache.
        let cache = memorisee(RESET, ANCRE, Some("opus"));
        let ancrage = calculer_ancrage(
            &fraiche(RESET),
            Some(&cache),
            MAINTENANT,
            Some("fable"),
            false,
        );
        assert_eq!(ancrage.observe_a, ANCRE);
        assert_eq!(ancrage.observe_modele, None);

        let ancrage = calculer_ancrage(&fraiche(RESET), None, MAINTENANT, Some("fable"), false);
        assert_eq!(ancrage.observe_a, MAINTENANT);
        assert_eq!(ancrage.observe_modele, None);
    }

    #[test]
    fn les_trois_reancrages_d_origine_etiquettent_la_famille_du_jour() {
        // Autre fenêtre : l'ancrage neuf porte la famille courante, pas celle
        // du cache.
        let cache = memorisee(RESET - 3_600, ANCRE, Some("opus"));
        let ancrage = calculer_ancrage(
            &fraiche(RESET),
            Some(&cache),
            MAINTENANT,
            Some("fable"),
            true,
        );
        assert_eq!(ancrage.observe_a, MAINTENANT);
        assert_eq!(ancrage.observe_modele.as_deref(), Some("fable"));

        // Sans cache du tout : idem.
        let ancrage = calculer_ancrage(&fraiche(RESET), None, MAINTENANT, Some("opus"), true);
        assert_eq!(ancrage.observe_a, MAINTENANT);
        assert_eq!(ancrage.observe_modele.as_deref(), Some("opus"));
    }

    /// La place hebdomadaire va à la fenêtre du modèle — la nouvelle venue,
    /// celle de `/usage` —, sauf quand la globale est en alerte et plus remplie.
    #[test]
    fn la_place_va_a_la_fenetre_du_modele_sauf_alerte_globale() {
        let candidat = |pourcent: i64, en_alerte: bool| {
            let palier = if en_alerte {
                Palier::Alerte
            } else {
                Palier::Aucun
            };
            Candidat {
                segment: format!("7j {}%", pourcent),
                pourcent,
                palier,
                pire: palier,
            }
        };
        let pourcent = |place: &Option<Candidat>| place.as_ref().map(|c| c.pourcent);

        // Globale sans alerte : la fenêtre du modèle prend la place, qu'elle
        // soit plus remplie (le 03/09/2026 : 28 contre 14)…
        let mut place = Some(candidat(14, false));
        retenir(&mut place, Some(candidat(28, false)));
        assert_eq!(pourcent(&place), Some(28));

        // … ou moins (le 04/09/2026, lendemain de remise à zéro : 0 contre 2).
        let mut place = Some(candidat(2, false));
        retenir(&mut place, Some(candidat(0, false)));
        assert_eq!(pourcent(&place), Some(0));

        // Globale en alerte et plus remplie : elle garde la place, c'est elle
        // qui menace.
        let mut place = Some(candidat(60, true));
        retenir(&mut place, Some(candidat(28, false)));
        assert_eq!(pourcent(&place), Some(60));

        // Globale en alerte mais moins remplie : la fenêtre du modèle, qui
        // alerte davantage encore.
        let mut place = Some(candidat(60, true));
        retenir(&mut place, Some(candidat(80, true)));
        assert_eq!(pourcent(&place), Some(80));

        // Égalité, alerte ou non : la nouvelle venue.
        let mut place = Some(candidat(60, true));
        retenir(&mut place, Some(candidat(60, false)));
        assert_eq!(place.as_ref().map(|c| c.segment.as_str()), Some("7j 60%"));

        // Rien à proposer : l'occupant reste.
        retenir(&mut place, None);
        assert_eq!(pourcent(&place), Some(60));

        // Place vide : la première venue la prend.
        let mut vide: Option<Candidat> = None;
        retenir(&mut vide, Some(candidat(5, false)));
        assert_eq!(pourcent(&vide), Some(5));
    }

    /// Une fenêtre pas encore entamée — 0 %, sans remise à zéro, telle que
    /// l'API la rend le lendemain d'une remise à zéro — se mesure, se mémorise
    /// avec son échéance nulle, et s'affiche sans rythme ni heure.
    #[test]
    fn une_fenetre_sans_echeance_se_mesure_sans_rythme_ni_heure() {
        let fenetre = json!({ "used_percentage": 0, "resets_at": null });
        // Ancrage mémorisé sur une échéance connue : il ne peut pas tenir face
        // à une échéance nulle, et l'ancrage repart sans faire échouer la
        // mesure.
        let memorisee = memorisee(RESET, ANCRE, Some("fable"));
        let mut a_memoriser = Vec::new();

        let candidat = mesurer(
            &fenetre,
            &Mesure {
                descripteur: &FENETRE_MODELE,
                effectif: &FENETRE_MODELE,
                memorisee: Some(&memorisee),
                instant: MAINTENANT,
                famille: Some("fable"),
                ancienne: false,
                estimation: None,
            },
            &mut a_memoriser,
        )
        .expect("segment");

        assert_eq!(candidat.pourcent, 0);
        assert!(!candidat.en_alerte());
        assert!(candidat.segment.contains(FENETRE_MODELE.libelle));
        assert!(!candidat.segment.contains(PREFIXE_PROJECTION));
        assert!(!candidat.segment.contains(PREFIXE_EPUISEMENT));
        // Sans échéance, pas d'heure « HH:MM » non plus — et rien d'autre sur
        // la ligne ne porte de deux-points.
        assert!(!candidat.segment.contains(':'));

        assert_eq!(a_memoriser.len(), 1);
        assert_eq!(a_memoriser[0].resets_at, Value::Null);
        assert_eq!(a_memoriser[0].observe_a, Value::from(MAINTENANT));
        assert_eq!(a_memoriser[0].observe_pourcent, Value::from(0));
        assert_eq!(a_memoriser[0].observe_modele, Value::from("fable"));
    }

    /// Un relevé ancien s'affiche comme une valeur de cache — marqué, sans
    /// rythme — mais reste ancré et mémorisé : la mesure est réelle.
    #[test]
    fn un_releve_ancien_est_marque_et_memorise() {
        let fenetre = json!({ "used_percentage": 28, "resets_at": RESET });
        let mut a_memoriser = Vec::new();

        let candidat = mesurer(
            &fenetre,
            &Mesure {
                descripteur: &FENETRE_MODELE,
                effectif: &FENETRE_MODELE,
                memorisee: None,
                instant: MAINTENANT,
                famille: Some("fable"),
                ancienne: true,
                estimation: None,
            },
            &mut a_memoriser,
        )
        .expect("segment");

        assert_eq!(candidat.pourcent, 28);
        assert!(candidat.segment.contains("~28"));
        assert_eq!(a_memoriser.len(), 1);
        assert_eq!(a_memoriser[0].cle, FENETRE_MODELE.cle);
        assert_eq!(a_memoriser[0].observe_modele, Value::from("fable"));
        assert_eq!(a_memoriser[0].observe_a, Value::from(MAINTENANT));
    }

    /// Une estimation remplace le pourcentage relevé : marquée « ≈ », sans
    /// rythme, colorée et départagée sur la valeur estimée — et le cache garde
    /// le relevé, avec son ancrage.
    #[test]
    fn une_estimation_remplace_la_mesure_et_tait_le_rythme() {
        let fenetre = json!({ "used_percentage": 28, "resets_at": RESET });
        let memorisee = memorisee(RESET, ANCRE, Some("fable"));
        let mesure = |estimation: Option<i64>, ancienne: bool| Mesure {
            descripteur: &FENETRE_MODELE,
            effectif: &FENETRE_MODELE,
            memorisee: Some(&memorisee),
            instant: MAINTENANT,
            famille: Some("fable"),
            ancienne,
            estimation,
        };

        // Sans estimation, l'ancrage de deux jours donne un rythme : c'est
        // lui que l'estimation doit taire.
        let mut a_memoriser = Vec::new();
        let relevee = mesurer(&fenetre, &mesure(None, false), &mut a_memoriser).expect("segment");
        assert!(relevee.segment.contains(PREFIXE_PROJECTION));
        assert!(!relevee.en_alerte());

        let mut a_memoriser = Vec::new();
        let estimee =
            mesurer(&fenetre, &mesure(Some(32), false), &mut a_memoriser).expect("segment");
        assert_eq!(estimee.pourcent, 32);
        assert!(estimee.segment.contains(&format!("{}32", MARQUEUR_ESTIMEE)));
        assert!(!estimee.segment.contains(PREFIXE_PROJECTION));
        assert!(!estimee.segment.contains(MARQUEUR_MEMORISEE));
        // Le cache garde le relevé et son ancrage, jamais l'estimation.
        assert_eq!(a_memoriser.len(), 1);
        assert_eq!(a_memoriser[0].used_percentage, json!(28));
        assert_eq!(a_memoriser[0].observe_pourcent, Value::from(10));

        // Estimée, la valeur n'est plus ancienne : « ≈ » l'emporte sur « ~ ».
        let mut a_memoriser = Vec::new();
        let ancienne =
            mesurer(&fenetre, &mesure(Some(32), true), &mut a_memoriser).expect("segment");
        assert!(ancienne
            .segment
            .contains(&format!("{}32", MARQUEUR_ESTIMEE)));
        assert!(!ancienne.segment.contains(MARQUEUR_MEMORISEE));

        // L'alerte, et l'heure de remise à zéro qui l'accompagne, se jugent
        // sur l'estimation : 80 % la franchit là où le relevé à 28 % ne
        // portait pas d'heure.
        let mut a_memoriser = Vec::new();
        let en_alerte =
            mesurer(&fenetre, &mesure(Some(80), false), &mut a_memoriser).expect("segment");
        assert!(en_alerte.en_alerte());
        assert!(en_alerte.segment.contains(':'));
        assert!(!relevee.segment.contains(':'));
    }
}
