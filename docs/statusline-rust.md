# Portage de la ligne de statut en Rust — dossier

> **Statut : fait le 21/08/2026.** Le crate vit dans `statusline-rs/`, le
> binaire déployé sert la ligne de statut, et `refreshInterval` est descendu à
> une seconde. La recette est passée du premier coup : *aucune divergence sur
> 81 cas* face à `statusline.ps1`.
>
> Rédigé le 14/08/2026 sur Claude Code 2.1.232 comme dossier d'instruction,
> avec `cargo`/`rustc` 1.97.0. Exécuté le 21/08/2026 sur 2.1.238, `cargo`/
> `rustc` 1.97.1. Les sections 1 à 10 sont conservées **telles qu'elles ont été
> écrites avant le portage** : elles disent ce qui avait été prévu, et la
> section 11 dit ce qui s'est passé. Ce qu'elles annoncent au futur se lit donc
> comme un plan, non comme l'état courant.
>
> L'usage courant — recompiler, redéployer, rejouer la recette — est documenté
> dans `README.md`, section « Portage en Rust ». Ce fichier-ci garde les
> mesures et le raisonnement.

Le fonctionnement de la ligne de statut, ses segments et son cache sont
documentés dans `README.md`, section « Contenu de la ligne de statut ». Le
présent fichier ne les répète pas : il ne traite que du changement de langage.

---

## 1. Pourquoi la question se pose

`statusline.ps1` coûte **620 ms par rafraîchissement**. La ligne affichée retarde
donc d'un peu plus d'une demi-seconde sur l'état réel de la session, et chaque
événement déclenche 620 ms de CPU. Un binaire natif ramènerait ce coût à une
quinzaine de millisecondes.

C'est le seul reproche mesurable qu'on puisse faire au script. Sur tout le
reste — dégradation, hiérarchie visuelle, couverture de test, documentation des
décisions — il n'y a rien à reprendre.

---

## 2. Mesures

### Protocole

Cinq à dix tirs par variante, médiane retenue, `%LOCALAPPDATA%` détourné vers un
dossier jetable pour ne pas polluer l'ancrage du cache réel (voir la mémoire de
session `statusline-test-cache-reel`). Toutes les variantes sont lancées depuis
PowerShell via `Measure-Command`, donc avec la même surcouche : la comparaison
est homogène, même si les valeurs absolues surestiment légèrement le coût nu du
processus.

### Résultats

| Variante | Coût par rafraîchissement |
|---|---|
| `pwsh` + `statusline.ps1` (en place) | **620 ms** |
| Python 3.12, à titre de repère | ~73 ms |
| Binaire Rust témoin, sur `C:` | **13,3 ms** |
| Le même binaire, lancé depuis `H:` | ~20 ms à chaud, **119 ms à froid** |

Soit un facteur **~47×** entre l'existant et la cible.

### Décomposition des 620 ms

| Poste | Coût |
|---|---|
| Démarrage de `pwsh -NoProfile`, à vide | 288 ms |
| Parsing et compilation des 53 Ko du script | +110 ms |
| Exécution : JSON, cmdlets et types .NET au premier appel | +222 ms |

Détail du dernier poste, mesuré dans un `pwsh` neuf : `ConvertFrom-Json` 38 ms,
`Get-Item .VersionInfo` 14 ms, `Test-Path` 12 ms, `DateTimeOffset` 6 ms,
`ConvertTo-Json` 5 ms. Le reste se disperse dans la définition des trente
fonctions et le premier appel de chaque cmdlet.

### Ce que la mesure invalide

**Le lecteur réseau n'est pas en cause.** C'était l'hypothèse naturelle, elle est
fausse :

| Lecture soupçonnée | Coût réel |
|---|---|
| Remontée `.git` sur `H:` (32 niveaux, hors dépôt) | **4,8 ms** |
| Métadonnées de `claude.exe` (319 Mo) | **2,6 ms** |

Il n'y a donc **aucune optimisation locale à aller chercher** dans le script :
son coût est celui de PowerShell, pas celui de ses accès disque. Élaguer les
commentaires gagnerait la centaine de millisecondes de parsing au prix de ce qui
fait la valeur du fichier — mauvais échange, écarté.

Cette mesure a une conséquence directe sur une décision déjà consignée : voir
§ 8.

### Ce que la mesure ne dit pas

**Il n'y a pas de délai serré côté Claude Code.** La constante de classement des
échecs de la ligne de statut vaut `600000` ms, soit dix minutes, avant qu'un
lancement soit compté en `timeout` (repérée dans le binaire 2.1.232, aux côtés
de `spawn_failed` et `nonzero_exit`). Le script actuel ne risque donc pas d'être
tué, et rien ne presse.

Le gain est réel, mais il est d'un seul ordre : supprimer un demi-seconde de
retard d'affichage. **Si ce retard ne gêne pas à l'usage, la réécriture est un
confort, pas une nécessité.** C'est le véritable arbitrage, et il n'appartient
pas à ce document de le trancher.

---

## 3. Socle disponible

- `cargo 1.97.0`, `rustc 1.97.0` installés sur le poste.
- `PyScripts\PowerShell\RustTools\` fournit la convention maison : profil release
  déjà réglé (`opt-level = 3`, `lto`, `codegen-units = 1`, `strip`), plusieurs
  binaires dans un crate, script `build.ps1`.
- **`serde`, `serde_json`, `chrono` et `windows-sys` sont déjà présents dans le
  cache cargo local** (`%USERPROFILE%\.cargo\registry\cache`). La compilation se
  fait en `--offline`, sans rien télécharger — ce qui respecte `AGENTS.md` §1.5.

Ce sont les quatre seules dépendances nécessaires. Ni `clap` (le programme ne
prend aucun argument), ni `anyhow` (tout se traite en `Option`, conformément à la
règle « rien n'est jamais fatal »).

---

## 4. Emplacement du crate

**Proposition : `.claude\user-config\statusline-rs\`**, à côté de la
configuration qu'il sert.

Trois raisons de **ne pas** l'ajouter comme binaire de `RustTools` :

1. `panic = "abort"` y est déclaré au niveau du crate et supprimerait le filet de
   sécurité de la ligne de statut (voir § 5 b) ;
2. on hériterait du temps de compilation de `rayon`, `clap` et `thiserror`, sans
   en utiliser aucun ;
3. l'outillage Claude Code vit sous `.claude/` (`.claude/CLAUDE.md` § 8), pas
   dans les outils WindowsManagement.

> **Attention à `target/`.** Le workspace est sur un lecteur Google Drive. Poser
> un `.cargo\config.toml` dans le crate avec `build.target-dir` sous
> `%LOCALAPPDATA%`, sinon chaque compilation synchronise des centaines de
> mégaoctets d'artefacts.

---

## 5. Les six pièges du portage

Deux d'entre eux ont été vérifiés au prototype, les autres sont identifiés par
lecture du script existant.

### a. L'arrondi diverge, silencieusement — **vérifié**

`[math]::Round` de PowerShell fait un arrondi **au pair** (dit « du banquier ») ;
`f64::round()` de Rust arrondit à l'opposé de zéro.

| Valeur | PowerShell | `f64::round()` | `f64::round_ties_even()` |
|---|---|---|---|
| 2.5 | 2 | 3 | 2 |
| 3.5 | 4 | 4 | 4 |
| 0.5 | 0 | 1 | 0 |

Le cas n'est pas théorique : `rate_limits.<fenêtre>.used_percentage` vaut
`utilization × 100` côté Claude Code, donc un `.5` exact est atteignable.

**Parade : `f64::round_ties_even()`** (stable depuis Rust 1.77), partout où
`ConvertTo-Pourcent` intervient. Reproduit PowerShell exactement.

### b. `panic = "abort"` supprimerait le filet de sécurité

Toute la philosophie du fichier tient dans le `catch` final qui replie sur le nom
du modèle : **une sortie vide efface la ligne** dans l'interface — c'est le
constat du 02/08/2026 consigné dans `README.md`. Avec `panic = "abort"`,
`catch_unwind` est inopérant et un panic ne produit aucune sortie du tout.

**Parade :** crate séparé, en déroulement classique (`panic = "unwind"`), avec un
`catch_unwind` autour de l'assemblage et le même repli sur le nom du modèle. Le
typage fait le reste : pas d'indexation nue, pas d'`unwrap`, arithmétique
vérifiée.

### c. Le cache doit être écrit octet pour octet à l'identique

`test-statusline.ps1` compare le contenu du cache après chaque cas. Trois
propriétés à reproduire, sous peine de 80 divergences :

- l'ordre des clés (`[ordered]@{}` côté PowerShell) ;
- les champs absents écrits `null`, jamais omis ;
- aucune espace (`ConvertTo-Json -Compress`).

Un `struct` sérialisé par `serde_json::to_string` donne exactement cela, à
condition de déclarer les champs dans l'ordre et de garder les `Option` en
`null`.

### d. La version se lit dans `StringFileInfo`, pas dans `VS_FIXEDFILEINFO` — **vérifié**

PowerShell lit `.VersionInfo.ProductVersion`, qui n'est pas la structure
numérique fixe mais **la chaîne du bloc `StringFileInfo`**. Un portage sur les
champs numériques peut diverger d'un binaire à l'autre.

Il faut donc passer par `\VarFileInfo\Translation` pour obtenir la langue et la
page de codes réellement présentes, avant d'interroger
`\StringFileInfo\{langue}{page}\ProductVersion`. Prototype écrit et exécuté :

```
ProductVersion = Some("2.1.232.0")  (lu en 598 µs)
```

Identique à ce que rend PowerShell (`'2.1.232.0'`, que `Get-VersionBinaire`
rabote ensuite en `2.1.232`), et **4× plus rapide** que les 2,6 ms du cmdlet.

Le prototype, à conserver — il est la partie du portage la moins évidente à
retrouver :

```rust
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};

fn large(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Rend la chaîne ProductVersion du bloc StringFileInfo, comme le fait
/// FileVersionInfo.ProductVersion côté .NET, ou None si elle manque.
fn product_version(chemin: &str) -> Option<String> {
    let fichier = large(chemin);
    unsafe {
        let taille = GetFileVersionInfoSizeW(fichier.as_ptr(), std::ptr::null_mut());
        if taille == 0 {
            return None;
        }
        let mut bloc = vec![0u8; taille as usize];
        if GetFileVersionInfoW(fichier.as_ptr(), 0, taille, bloc.as_mut_ptr().cast()) == 0 {
            return None;
        }

        // Langue et page de codes effectivement présentes dans le fichier.
        let mut trad: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut n: u32 = 0;
        let cle = large("\\VarFileInfo\\Translation");
        if VerQueryValueW(bloc.as_ptr().cast(), cle.as_ptr(), &mut trad, &mut n) == 0 || n < 4 {
            return None;
        }
        let langue = *(trad as *const u16);
        let page = *(trad as *const u16).add(1);

        let sous_bloc = format!("\\StringFileInfo\\{:04x}{:04x}\\ProductVersion", langue, page);
        let cle = large(&sous_bloc);
        let mut valeur: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut longueur: u32 = 0;
        if VerQueryValueW(bloc.as_ptr().cast(), cle.as_ptr(), &mut valeur, &mut longueur) == 0
            || longueur == 0
        {
            return None;
        }
        let brut = std::slice::from_raw_parts(valeur as *const u16, longueur as usize);
        let fin = brut.iter().position(|&c| c == 0).unwrap_or(brut.len());
        Some(String::from_utf16_lossy(&brut[..fin]))
    }
}
```

Dépendance : `windows-sys = { version = "0.61", features = ["Win32_Storage_FileSystem", "Win32_Foundation"] }`.

### e. Les largeurs se comptent en caractères, pas en octets

`$Valeur.Length` compte des unités UTF-16 ; `str::len()` compte des octets. Sans
`chars().count()`, le cadrage de `Format-Mesure` se décalerait sur `épuisé` et
sur les chemins accentués — le harnais couvre justement `données\été`.

### f. La comparaison de chemins doit rester insensible à la casse en Unicode

`Format-Repertoire` utilise `OrdinalIgnoreCase`, qui traite `DONNÉES` et
`données` comme égaux. `eq_ignore_ascii_case` ne le ferait pas : passer par
`to_lowercase()`, qui est Unicode en Rust.

### Ce qui disparaît, à l'inverse

- Toute la justification de `Write-LigneStatut` — page de codes OEM,
  `$PSStyle.OutputRendering` — se réduit à `stdout().write_all(octets)`.
- `fs::rename` sur Windows a exactement la sémantique de remplacement atomique
  de `Move-Item -Force` : l'écriture du cache par fichier temporaire renommé se
  transpose telle quelle.
- Les conversions défensives (`ConvertTo-Nombre`, `ConvertTo-Instant`) restent
  nécessaires, mais le typage de `serde_json::Value` en fait un `match` explicite
  au lieu d'un `try/catch`.

> **Point de vigilance sur `ConvertTo-Nombre`.**
> `[System.Convert]::ToDouble` est plus permissif que `str::parse::<f64>()` : il
> accepte les espaces encadrants et les booléens. Il faut donc traiter
> explicitement les variantes `Number`, `String` (après `trim()`) et `Bool` de
> `serde_json::Value` pour ne pas perdre de cas couverts par le harnais.

---

## 6. Recette : le harnais est déjà l'oracle

C'est l'argument le plus fort en faveur du portage. `test-statusline.ps1` soumet
**80 cas** au script et observe la ligne écrite **et** l'état du cache. Il lui
manque environ cinq lignes : dans `Invoke-Cas`, si le candidat se termine par
`.exe`, l'invoquer directement au lieu de le passer à `pwsh`. Ensuite :

```powershell
pwsh -ExecutionPolicy Bypass -File .claude\user-config\test-statusline.ps1 `
    -Candidat <...>\statusline.exe -Reference .claude\user-config\statusline.ps1
```

Le critère d'acceptation devient littéralement **« Aucune divergence sur 80
cas »**. Les quatre cas `binaire …` détournent déjà `CLAUDE_STATUSLINE_BINAIRE`
et fonctionnent sans modification.

S'y ajoutent des tests `#[cfg(test)]` sur les fonctions pures — conversions,
`Format-Rythme`, `Get-Ancrage` — que `AGENTS.md` § 6 réclame et que le harnais ne
couvre qu'indirectement.

> **Séquencement, et il n'est pas négociable.** Porter d'abord **à
> iso-comportement**, prouver « aucune divergence », déployer. *Ensuite*
> seulement ajouter les évolutions fonctionnelles envisagées (racines
> `worktree`/`added_dirs`, ancrage du rythme sur le flottant), côté Rust, avec de
> nouveaux tests. Mélanger les deux ferait perdre l'oracle, qui est le seul filet
> disponible dans un workspace sans git.

---

## 7. Déploiement

L'exécutable doit vivre sur `C:` — les 119 ms à froid depuis `H:` mesurés en § 2
le disent assez.

- Compilation vers `%LOCALAPPDATA%`, dépôt en
  `%USERPROFILE%\.claude\statusline.exe`.
- `settings.json` passe de `pwsh -NoProfile … -File …` au chemin de l'exécutable,
  en conservant `"padding": 0`.
- `sync-user-config.ps1` fonctionne par liste blanche de **fichiers versionnés** ;
  un exécutable est un artefact de compilation. Lui ajouter une branche dédiée
  « compiler puis déployer » plutôt que de versionner un binaire de plusieurs
  centaines de kilo-octets sur Google Drive à chaque build.
- L'écrasement d'un exécutable en cours d'exécution peut échouer en partage. La
  fenêtre est de 13 ms, mais le déploiement doit copier en `.new` puis renommer,
  ou réessayer.

---

## 8. Effet sur une décision déjà consignée : `refreshInterval`

`README.md`, section « Réglages envisagés, non appliqués », écarte
`refreshInterval` au motif que la remontée `.git` sur le lecteur réseau serait
trop coûteuse sur minuterie courte.

**La mesure de § 2 retire cet argument : cette remontée coûte 4,8 ms.** Ce qui
coûte, c'est le démarrage de `pwsh` — 620 ms au total. Deux conséquences :

- en l'état, `refreshInterval` reste discutable, mais pour une autre raison que
  celle consignée ;
- **après portage en Rust, l'objection tombe entièrement** : à 13 ms le
  rafraîchissement, une minuterie de 30 à 60 secondes devient sans effet notable,
  et les trois informations qui vieillissent au repos — heure de remise à zéro,
  projection, heure d'épuisement — cesseraient de dériver.

C'est un bénéfice indirect du portage qu'il serait dommage d'oublier au moment
de trancher.

> **Après coup.** Ce « bénéfice indirect » s'est révélé être le motif principal,
> et il est allé bien au-delà d'une minuterie de 30 à 60 secondes : le réglage
> est à **une seconde**, le plancher accepté par Claude Code. Voir §11, et
> `README.md` pour le détail du plancher — qui n'est pas seulement une affaire
> de dépense CPU, un intervalle plus court que le programme faisant tuer chaque
> lancement par le suivant.

---

## 9. Coût, risques, ce qu'on perd

**Charge estimée : 2 à 3 jours.**

| Lot | Charge |
|---|---|
| Squelette du crate, E/S, lecture du payload | ½ j |
| Portage des segments, du rythme et de l'ancrage | 1 j |
| Version `StringFileInfo`, fuseau horaire, tests unitaires | ½ j |
| Recette croisée, déploiement, documentation | ½ j |

L'essentiel du temps n'est pas le code : c'est le report des quelque
600 lignes de commentaires, qui sont la vraie valeur du fichier et doivent
survivre en `///`.

Ce qu'on perd :

- régler un seuil demande une recompilation (10 à 30 s) au lieu d'une édition
  suivie d'un `sync-user-config.ps1` ;
- le débogage devient moins immédiat qu'un script lisible en clair sur le poste.

En contrepartie, si la toolchain casse, l'exécutable déjà déployé continue de
tourner : la ligne de statut ne dépend pas de la présence de `cargo`.

---

## 10. Questions ouvertes

1. **`NO_COLOR` pour la ligne de statut.** La question posée en fin de `README.md`
   n'est pas tranchée : si Claude Code définit `NO_COLOR=1` pour la ligne de
   statut comme il le fait pour les sous-processus, tous les seuils de coloration
   sont sans effet visible. À vérifier **avant** le portage — c'est le genre de
   chose qui change ce qu'on porte.
2. **`subagentStatusLine`.** Le binaire 2.1.232 expose, à côté de `statusLine`,
   une clé de configuration `subagentStatusLine` : une ligne de statut distincte
   pour les sessions de sous-agent. Le workspace en utilise
   (`readme-first-explorer`). Un binaire natif servirait les deux pour le même
   prix — à instruire séparément.
3. **Dump du payload.** Un mode `CLAUDE_STATUSLINE_DEBUG=payload`, qui
   journaliserait une fois le payload brut, rendrait vérifiable le contrat
   d'entrée réel plutôt que déduit de la lecture du binaire. Utile avant de figer
   des `struct` Rust, et utile à chaque évolution de Claude Code.

   > **Fait le 25/08/2026**, par fichier témoin plutôt que par variable
   > d'environnement — une variable ne se lit qu'au lancement de Claude Code,
   > alors que le besoin naît dans la session ouverte. Module `capture.rs`,
   > mode d'emploi dans `README.md`, section « Capture du payload ». Le premier
   > relevé a immédiatement écarté une évolution qui paraissait acquise :
   > `permission_mode` n'est pas dans le payload.

---

## 11. Décision, puis exécution

### La décision

Prise le 21/08/2026, et **pas pour la raison instruite ici**. Le dossier pesait
un demi-seconde de retard d'affichage contre deux à trois jours de travail, et
concluait au confort. C'est une autre demande qui a tranché : que la ligne *ne
disparaisse jamais et se rafraîchisse au fil de l'eau*.

Cette exigence-là ne se satisfait qu'en abaissant `refreshInterval`, seul filet
contre ce que le programme ne contrôle pas — `spawn_failed`, tube rompu avant
que le JSON n'arrive. Or l'intervalle ne peut pas descendre sous la durée du
programme : chaque lancement annule le précédent (`CFc.#_()` appelle `abort()`,
après 300 ms de débours), si bien qu'un intervalle trop court gèlerait la ligne
au lieu de la rafraîchir. À 626 ms, PowerShell interdisait la seconde ; à
12,5 ms, le binaire l'autorise pour 1,25 % d'un cœur — **moins que ce que
coûtait le réglage à 30 secondes**.

Le portage n'était donc plus un confort : il était la condition.

### Ce qui s'est passé

Environ deux heures, et non deux à trois jours. L'écart tient à la qualité de
l'instruction : les six pièges de la §5 étaient tous justes, le prototype
`ProductVersion` a été repris tel quel, et le harnais a fait exactement l'office
d'oracle qu'on attendait de lui.

| Étape | Résultat |
|---|---|
| Cinq lignes du harnais (`Invoke-Cas` lance un `.exe` directement) | conforme au plan |
| Crate, dépendances, compilation `--offline` | 11 s, rien à télécharger |
| Portage des 8 sections | ~1 200 lignes, commentaires reportés |
| Tests unitaires sur les fonctions pures | 7 tests |
| Recette croisée sur 81 cas | **aucune divergence, au premier essai** |

### Trois surprises, dont deux corrections au dossier

**a. `TryParse("20260802")` rend `False`.** Le commentaire de `ConvertTo-Instant`
affirmait qu'une chaîne datée compacte se lirait comme une date, et justifiait
par là l'ordre des branches. Vérification faite, .NET refuse ce format : une
telle chaîne retombe sur la voie numérique. L'ordre des branches reste correct,
mais sa justification était fausse — et le portage n'a donc pas eu à
implémenter `yyyyMMdd`, ce qui aurait au contraire **créé** une divergence.

**b. `cargo metadata` ne lit pas `.cargo/config.toml` depuis `--manifest-path`.**
Cargo remonte depuis le **répertoire courant**, jamais depuis le manifeste.
Interrogé depuis la racine du workspace, il annonçait le `target/` par défaut —
sur le lecteur réseau — que la compilation n'utilise pas, et le script de
déploiement cherchait le binaire là où il n'était pas, en signalant
« compilation en échec (code 0) ». Même classe de piège que le `Glob` qui suit
le `Set-Location`.

**c. Le binaire devait être versionné.** La §7 recommandait une branche
« compiler puis déployer » plutôt que de versionner l'exécutable. C'était
sous-estimer ce que promet ce dossier : *tout* doit se restaurer depuis
`.claude\` seul. Une machine sans `cargo` n'aurait rien pu compiler, et le
`settings.json` restauré aurait pointé un exécutable absent — soit une ligne de
statut vide, très exactement le symptôme que le portage supprimait. Les 330 Ko
sont donc versionnés à côté des sources, la compilation ne servant qu'à les
rafraîchir. Rejoué `cargo` retiré du `PATH` : restauration complète, ligne
affichée.

### Ce qui reste ouvert

Deux des trois questions de la §10 sont intactes — `subagentStatusLine` et le
mode de dump du payload. La première, `NO_COLOR` pour la ligne de statut, est
**tranchée depuis le 21/08/2026 : la ligne n'en hérite pas**, la coloration
s'affiche donc réellement. Le journal de diagnostic a suffi à l'établir, sans
l'instrumentation qu'envisageait la §10 : sa longueur `n` compte les séquences
ANSI. Voir `README.md`, section « Coloration ». Les sous-processus, eux, en
héritent bel et bien — un `cargo test` du crate compris, ce qui est un piège
pour tout test de coloration écrit sans précaution.

S'y ajoutent
maintenant les évolutions fonctionnelles que le séquencement de la §6 avait
mises en attente, et qui se font désormais côté Rust : racines
`workspace.added_dirs`, ancrage du rythme sur le flottant, et les champs
apparus depuis 2.1.226 dans le payload (`session_name`, `output_style`,
`vim.mode`, `agent.name`, `pr`, `workspace.git_worktree` — ce dernier
rendrait peut-être la remontée `.git` inutile).

## 12. Deux évolutions, et ce qu'elles ont coûté — 25/08/2026

Les premières faites depuis le portage, côté Rust seul, le script restant
oracle et repli. Aucune divergence sur les 81 cas : le marqueur demande un
`context_window_size` qu'aucun cas ne porte, et la capture est inerte sans son
témoin. Leur couverture appartient donc aux tests du crate — 22 tests, contre 9
avant.

| Évolution | Module | Ce qu'elle change |
|---|---|---|
| Capture du payload | `capture.rs`, neuf | Relève un payload brut sur témoin, puis se désarme |
| Marqueur de fenêtre | `segments.rs` | `Opus 5 1M xhigh` hors de la fenêtre ordinaire |

Une troisième modification, invisible : l'écriture par temporaire renommé est
passée de `cache.rs` à `disque.rs` (`ecrire_atomique`), la capture en ayant le
même besoin. Le nom du fichier temporaire est conservé à l'identique, extension
comprise — le harnais compare l'état du dossier de cache.

**L'ordre des deux comptait.** La capture a été écrite d'abord, et le premier
relevé a servi à trancher le second point : la session tournait en
`context_window_size: 1000000` avec `exceeds_200k_tokens: true`, ce qui a
départagé les deux champs candidats sur pièces plutôt qu'au raisonnement. Il a
surtout écarté une troisième évolution qu'on s'apprêtait à instruire — un
segment de mode de permission — en montrant que `permission_mode` n'est pas
dans le payload.

## 13. Six retouches d'affichage, et la règle du §6 mise en défaut — 26/08/2026

Instruites comme propositions graphiques chiffrées en colonnes, retenues en bloc,
appliquées le lendemain. Le détail de chacune vit dans `README.md`, section « Six
retouches » ; ce qui appartient à ce dossier-ci est ce qu'elles ont appris sur la
méthode.

### Ce que la mesure a trouvé avant que rien ne soit dessiné

Deux vérifications préalables, l'une et l'autre payantes :

**a. Le point médian n'était pas coloré.** La sortie brute, relue séquence par
séquence, montre le ` · ` hors de toute séquence SGR. Le rang intermédiaire de
l'échelle sortait donc dans la couleur pleine du thème, plus clair que la barre
du rang supérieur et que les libellés qu'il sépare. Trois documents décrivaient
cette échelle, aucun ne l'avait vérifiée sur la sortie.

**b. Six des huit blocs de la jauge n'étaient pas dans Consolas.** La table de
glyphes de la police, lue par `GlyphTypeface.CharacterToGlyphMap`, dit que `▄` et
`█` y sont et que `▁▂▃▅▆▇` n'y sont pas — contre un commentaire du code qui
affirmait le bloc « Block Elements » complet. Le terminal les faisait dessiner
par une police de repli, à une autre graisse et une autre assise ; la jauge
changeait de main au milieu de son échelle.

> La leçon est la même dans les deux cas, et c'est celle du §5 : **une propriété
> d'affichage se relève, elle ne se suppose pas.** Le portage avait déjà corrigé
> une affirmation fausse par la mesure — `TryParse("20260802")` rend `False`. Il
> en restait deux, dans le domaine le plus difficile à vérifier de tête.

### La règle du §6 ne couvrait pas ce cas

Le séquencement du §6 conclut que les évolutions se font côté Rust seul, le
script restant oracle. Le §12 le vérifie : *aucune divergence*, parce qu'aucun
cas du harnais n'exerçait les deux évolutions d'alors.

Ces six retouches-là, les 81 cas les exercent **tous**. La règle appliquée
littéralement n'aurait pas entamé l'oracle, elle l'aurait supprimé : 81
divergences voulues, plus aucun écart involontaire discernable au milieu. Le
critère utile n'est donc pas « Rust seul » mais celui qu'énonçait déjà le
commentaire de `CODE_PASTILLE` : de la **mise en forme** se porte des deux côtés,
de la **logique** non.

Les six sont de la mise en forme pure, dont deux ne sont que des constantes. La
transcription a tenu du premier coup.

| Étape | Résultat |
|---|---|
| Portage Rust : 5 modules touchés, 2 constantes neuves | `CODE_SEPARATEUR`, `CODE_MARQUEUR`, `UNITE_POURCENT` |
| Tests du crate | 28, contre 22 avant |
| Transcription dans l'oracle PowerShell | **aucune divergence sur 81 cas**, au premier essai |
| Longueur de la ligne | 88 → 81 colonnes au calme, 103 → 95 en alerte |

### Ce qu'il reste à juger à l'écran

Trois points que ni la table de glyphes ni la sortie brute ne départagent, et
qui demandent une capture du terminal réel en corps 8 *light* sous acrylic :

1. **240 ou 245 pour le point médian.** 240 a été retenu par symétrie avec la
   barre ; à cette taille, le point pourrait y disparaître tout à fait.
2. **`adjustIndistinguishableColors` vaut `always`** dans le `settings.json` du
   terminal. Il se réserve donc d'éclaircir un premier plan trop proche du fond.
   Si l'ajustement porte aussi sur la palette 256, l'écart 240/245 réglé ici
   n'est pas celui qui s'affiche, et tout le travail sur la hiérarchie des gris
   se juge sur un rendu que le programme ne contrôle pas.
3. **Le cyan 109 des marqueurs.** 110 le rendrait plus bleu, 108 plus vert et
   donc plus proche de la pastille.

### Deux évolutions écartées, et pourquoi

- **La pastille en liseré** (`▐PY_xl▌`, demi-blocs dans la couleur du fond) :
  tient dans Consolas, coût nul, mais touche au seul aplat de la ligne. Reportée
  faute d'être jugeable autrement qu'à l'écran.
- **La barre proportionnelle à deux cellules** (`▊▏`, seize crans) : exige de
  passer `font.face` de Consolas à Cascadia Mono. C'est le seul changement de la
  série qui ne se décide pas dans le code, et il rouvrirait aussi les huit crans
  de hauteur. Le scan du binaire, lui, laissait croire le contraire.

## 14. Quatre retouches de plus, le même jour — 26/08/2026, second lot

Les trois points laissés « à juger à l'écran » en §13 ont été instruits comme les
six précédentes, mais avec un outil que la première série n'avait pas : une
**planche de comparaison** qui compose les variantes avec les vraies séquences
SGR et les affiche dans le terminal réel, en corps 8 *light* sous acrylic. Six
propositions, quatre retenues.

| Retouche | Ce qu'elle change | Colonnes |
|---|---|---|
| Point médian à 245 | il partageait 240 avec la barre | ±0 |
| Abonnement sans « Claude » | table **et** repli hors table | −7 |
| Version teintée sur l'écart | cyan 109 quand disque ≠ session | ±0 |
| Jauge à deux cellules | sept crans au lieu de quatre | +2 |

**82 colonnes au calme**, contre 87. Tests du crate : 37, contre 28. *Aucune
divergence sur 87 cas*, au premier essai.

### Ce que la mesure a de nouveau trouvé avant le dessin

Deux vérifications préalables, dans la ligne de celles du §13 :

**a. `▐` et `▌` sont bien dans Consolas.** Le liseré écarté le matin même tenait
donc, et tient toujours : il reste disponible, faute d'avoir été retenu. Les
blocs partiels `▏▎▍▌▋▊▉` n'y sont toujours pas — la jauge à seize crans reste
suspendue au changement de `font.face`, exactement comme le §13 le disait.

**b. Le terminal se réserve d'écraser l'arbitrage.**
`adjustIndistinguishableColors` vaut `"always"` sur ce poste : le §13 le
soupçonnait, la lecture du `settings.json` le confirme. Si l'ajustement porte
aussi sur la palette 256, l'écart 240/245 réglé ici n'est pas celui qui
s'affiche. C'est pourquoi la planche embarque une **mire de gris** 236→250 : le
seul moyen de trancher est de regarder, et la question ne se referme pas dans le
code.

Relevé au passage, sans rapport avec les retouches : `opacity` vaut **90** dans
le profil par défaut, là où `README.md` écrivait 80 depuis le 25/08/2026.

### Sept crans, et non huit

La proposition annonçait huit crans ; l'échelle en donne sept, et c'est elle qui
tranche. La cellule de gauche se remplit avant celle de droite — sémantique d'une
jauge, et lecture qui n'oblige pas à comparer deux densités entre elles. Avec
quatre densités par cellule, la chaîne **strictement croissante** que cela
autorise compte exactement sept crans : `░░ ▒░ ▓░ █░ █▒ █▓ ██`. Un huitième comme
`▒▒` ne se range ni avant ni après `█░` sans arbitraire, et l'arbitraire se
paierait sur la seule chose que la jauge sait faire — annoncer un ordre de
grandeur d'un coup d'œil.

`les_crans_de_jauge_sont_strictement_croissants` verrouille la propriété plutôt
que la table : une table future reste libre, la monotonie non.

### La règle du §6, une fois de plus

Les quatre sont de la mise en forme, et les 87 cas les exercent toutes — y
compris la teinte de version, ce qui ne se devinait pas : le socle du harnais
porte `version = "2.1.220"`, quand le binaire du poste est en 2.1.246. La
comparaison neuve était donc **vraie sur tous les cas ordinaires**, et n'écrire
que le Rust aurait fait diverger le harnais entier.

C'est la troisième fois que le critère « ce qui ferait diverger le harnais entier
se porte des deux côtés » décide, et la première où il fallait aller le vérifier
dans les données du harnais plutôt que le déduire de la nature du changement. La
teinte de version est de la **logique**, pas une constante d'affichage : la règle
du §6 la réservait au Rust. Elle a été portée quand même, et pour la seule raison
qui vaille — l'oracle disparaîtrait sinon.

### Ce qui reste ouvert

- **La pastille en tête de ligne**, écartée par l'utilisateur en même temps que
  le liseré. C'était la proposition qui corrigeait un défaut mesurable plutôt
  qu'un goût : l'ancre dérive de 21 colonnes selon les marqueurs affichés, donc
  bouge au moment précis où la session sort de l'ordinaire. Elle ne coûte rien en
  colonnes et reste applicable telle quelle.
- **La mire de gris n'a pas encore été lue.** Tant qu'elle ne l'est pas, on ne
  sait pas si le 245 appliqué ici se distingue du 240 de la barre à l'écran.

## 15. Le logo en tête de ligne, et ce que le binaire a livré — 26/08/2026

La demande était de reprendre le logo de l'écran de lancement, en plus petit,
et « que la couleur aussi soit fidèle ». Le détail du résultat vit dans
`README.md`, section « Le crabe en tête de ligne » ; ce qui appartient à ce
dossier-ci est la méthode et ce qu'elle a coûté.

### La source est le binaire, pas la mémoire

`claude.exe` embarque son bundle JS **en clair**, avec les caractères hors ASCII
échappés en `\uXXXX` — ce que la mémoire de session `contrat-claude-code-via-binaire`
disait déjà du contrat d'entrée. Un scan par blocs a donc rendu le composant
entier : le dessin, ses quatre poses (`default`, `look-left`, `look-right`,
`arms-up`), et surtout les deux couleurs, nommées `clawd_body` et
`clawd_background` dans les huit thèmes.

| Ce qu'on cherchait | Ce que le binaire dit |
|---|---|
| Le dessin | ` ▐▛███▛█` / `▝▜██████▀` / `  ▝▝ ▝▝`, en quadrants |
| La couleur du corps | `rgb(215,119,87)`, identique dans les six thèmes en couleurs vraies |
| La couleur du fond | `rgb(0,0,0)`, idem |
| Le repli 16 couleurs | `ansi:redBright` sur `ansi:black` |

Ce relevé tranche un point qu'aucun raisonnement n'aurait tranché : la couleur
du logo **n'est pas une couleur de thème**. Elle ne change ni entre clair et
sombre, ni sur les variantes daltonisées. La reprendre en couleurs vraies dans
une ligne qui tient en palette 256 partout ailleurs n'est donc pas une
inconséquence, c'est la seule lecture fidèle.

> **Deux fausses pistes, et le coût de la première.** Chercher les glyphes
> littéraux (`███`) dans le binaire ne rend **rien** : le bundle les échappe. Et
> un scan en PowerShell pur, octet par octet sur 250 Mo, ne termine pas dans un
> délai utile — il a fallu le tuer. Décoder chaque bloc en UTF-8 puis chercher
> par `IndexOf` ramène le balayage à quelques secondes, en Node comme en .NET.

### Ce que la mesure a de nouveau trouvé avant le dessin

Deux vérifications préalables, dans la ligne de celles des §13 et §14, et cette
fois **elles ont décidé du dessin lui-même** au lieu de l'ajuster :

**a. Le logo ne se réduit pas.** Sa densité vient des quadrants, quatre pixels
par cellule. Réduit d'un facteur trois en hauteur — le seul moyen de tenir sur
une ligne — son bitmap 18 × 6 échantillonne en une barre pleine : yeux et pattes
tombent tous sous le seuil de majorité, les six blocs de la rangée basse à 4/9
au mieux. Le calcul a été fait **avant** qu'aucune variante ne soit dessinée, et
il change la nature du travail : une version d'une ligne est un redessin, jamais
une réduction.

**b. Aucun des dix quadrants n'est dans Consolas.** `▖▗▘▝▙▚▛▜▞▟` : dix absences
sur dix, contre les huit présences de `▀▄█▌▐░▒▓`. Le logo affiché au lancement
est donc **déjà** rendu par une police de repli sur ce poste — ce que ni le §13
ni le §14 n'avaient eu l'occasion de constater, ayant relevé `▐` et `▌` sans
aller jusqu'aux quadrants. Reprendre le dessin caractère pour caractère aurait
importé ce défaut dans la ligne, au moment même où la jauge venait d'en être
tirée.

C'est la troisième fois que la table de glyphes décide, et la première où elle
interdit la solution évidente plutôt que de corriger une constante.

### La contrainte a rendu la technique

Privé des quadrants, il restait les demi-blocs verticaux et une propriété qu'on
n'avait pas encore employée : **une cellule porte deux pixels**, celui du haut
dans la couleur de fond, celui du bas dans celle de premier plan. La résolution
tombe à un pixel par cellule — la moitié de ce que donnent les quadrants — mais
elle suffit à une silhouette de cinq colonnes, et elle ne quitte jamais Consolas.

Le fond cesse alors d'accompagner le dessin : il **est** le dessin. Yeux, pattes
et coins ne sont rien d'autre que du fond laissé visible. D'où la seule décision
qui ne se déduisait pas du logo d'origine — sous `NO_COLOR`, la marque disparaît
entièrement au lieu de dégrader, `▀▄▀▄▀` sans ses deux teintes n'étant plus que
cinq blocs sans signification.

### La règle du §6, une quatrième fois

Le logo ne lit aucun champ du payload : il paraît donc sur **les 87 cas**, replis
compris. Le critère affiné au §14 — *ce qui ferait diverger le harnais entier se
porte des deux côtés* — s'applique sans hésitation, et pour la première fois à
quelque chose qui n'est ni de la mise en forme ni de la logique, mais un ornement.

| Étape | Résultat |
|---|---|
| Portage Rust : 3 modules touchés, 2 constantes neuves | `LOGO`, `CODE_LOGO` |
| Tests du crate | 40, contre 37 avant |
| Transcription dans l'oracle PowerShell | **aucune divergence sur 87 cas**, au premier essai |
| Longueur de la ligne | +6 colonnes, cinq de dessin et une d'espace |

Le point de pose demandait un choix : `terminer` côté Rust, `Write-LigneStatut`
côté script — le seul passage que **tous** les chemins traversent. Il est
délibérément **après** le contrôle de ligne blanche : posé avant, il rendrait non
blanche une ligne qui ne dit rien, et ferait taire le repli sur le nom du modèle
au moment précis où il sert. C'est l'invariant du §8 du programme principal, et
il aurait été perdu sans y penser.

### Six dessins dans la journée

`▀▄▀▄▀` le matin, `▀█▄█▄█▀` après la première capture d'écran, puis `▐▄█▄▌`,
`▝▄█▄▘` et `▝▄▀▄▘` sur la compacité — et `(V)°°(V)` le soir, qui quitte le
bitmap. Le logo déposé n'est donc ni le premier ni le plus fidèle : c'est le
sixième, et chaque passage a été décidé par quelque chose que le code ne pouvait
pas dire. La dernière section du chapitre revient sur ce que la série entière a
fini par apprendre.

### Deux corrections, à la première capture d'écran

Le dessin est passé de `▀▄▀▄▀` à `▀█▄█▄█▀` et le fond noir a disparu, une heure
après le dépôt. Les deux corrections viennent d'une capture du terminal réel, et
aucune des deux n'était visible autrement.

**a. Un damier n'est pas une silhouette.** Le premier dessin alternait haut et
bas à chaque cellule : deux yeux, deux pattes, coins ouverts — tous les éléments
y étaient, et rien n'y était continu. À l'écran, l'œil lisait un motif de
carreaux, pas une créature. La correction ne tient pas à un élément qui
manquait mais à l'ordre dans lequel on les pose : **le corps est plein d'abord**,
les creux ne viennent qu'ensuite y percer les yeux. Deux colonnes de plus
achètent cette masse continue, et les deux cellules de bord, réduites à leur
moitié haute, deviennent des pinces.

**b. Un noir en dur ne suit rien.** Reprendre `clawd_background` à la lettre
paraissait le comble de la fidélité. À l'écran, l'aplat `rgb(0,0,0)` devenait la
zone la plus sombre de la ligne : le terminal du poste tourne sous acrylic à
90 % d'opacité sur `#1E1E1E`, fond qui s'éclaircit quand une fenêtre claire
passe derrière, quand le noir absolu, lui, ne bouge pas. Le dessin se lisait
comme un rectangle étranger posé sur la ligne. Ne **rien** poser rend les creux
au fond réel, quel qu'il soit, et les fait suivre l'acrylic, l'opacité et le
thème sans qu'aucune valeur n'ait à être tenue à jour.

> C'est la limite du relevé dans le binaire, et elle méritait d'être notée :
> il donne la valeur juste — `rgb(0,0,0)`, sans ambiguïté — mais pas ce qu'elle
> devient dans un contexte qui n'est pas le sien. Le composant peint son ventre
> sur le fond de l'interface de Claude Code ; la ligne de statut, elle, est
> posée sur le terminal. La même valeur y change de rôle. **Fidèle à la lettre
> n'est pas fidèle à l'intention**, et seule la capture d'écran départage — ce
> que le §13 disait déjà d'une autre manière : une propriété d'affichage se
> relève, elle ne se suppose pas.

### La compacité, et ce qu'elle a rendu chiffrable

Sept colonnes ont paru larges, et la question « peut-on faire plus compact ? » a
obtenu une réponse plus nette que prévu, parce qu'elle se pose en termes
d'éléments et non de goût. Le dessin en porte trois — une **masse continue**,
deux **yeux**, deux **pinces** — et sept colonnes sont exactement le minimum
pour les tenir tous les trois.

| Largeur | Ce qui tient | Ce qui tombe |
|---|---|---|
| 7 colonnes | masse, yeux, pinces | — |
| **5 colonnes** | **masse, yeux, bords biseautés** | **les pinces** |
| 4 colonnes | masse, pinces | les deux yeux fusionnent en un creux |
| 3 colonnes | masse, pinces | un seul creux, forme abstraite |

Ce sont les pinces qui partent, et pas au hasard : elles ne tiennent qu'à deux
pixels isolés, quand la masse porte la forme et que les yeux portent le regard.
Un élément qui ne tient qu'à des pixels détachés est le premier à sacrifier, et
c'est le même constat que celui du damier vu par l'autre bout.

**Les demi-blocs latéraux ont rendu une part de la perte.** `▌` et `▐` étaient
là depuis le relevé du §14, notés présents dans Consolas et jamais employés :
ils portent une demi-cellule *pleine hauteur*, là où `▀` et `▄` portent un pixel
sur deux rangées. Aux deux bords, ils terminent la silhouette en biseau au lieu
de la couper net — un affinement qui ne coûte pas une colonne, puisqu'il occupe
une cellule qui devait de toute façon être là.

| Étape | Résultat |
|---|---|
| Portage Rust : 1 constante réécrite | `LOGO` |
| Tests du crate | 40, inchangés — la table de glyphes couvrait déjà `▌▐` |
| Transcription dans l'oracle PowerShell | **aucune divergence sur 87 cas**, au premier essai |
| Longueur de la ligne | 8 → 6 colonnes de marque |

> Le test `les_cellules_du_logo_sont_dans_consolas` n'a pas eu à bouger, sa
> liste ayant été écrite d'après le relevé complet plutôt que d'après les seuls
> glyphes employés. C'est le genre d'excès de portée qui se paie rarement et qui,
> ici, a rendu gratuit un changement de vocabulaire.

### Le seul repli de police accepté, et pourquoi il l'a été

Dernière retouche du jour : ouvrir les coins **bas** des cellules de bord tout
en gardant leur sommet affiné. C'est un **quart de bloc** — `▝` à gauche, `▘` à
droite — et la demande a buté sur une impossibilité nette.

`▐` est une demi-largeur *pleine hauteur* : rien ne permet d'en retirer le bas.
Une cellule ne porte que deux couleurs et une forme fixe, Consolas ne possède
que huit des trente-deux « Block Elements », et aucun d'eux n'est un quart. Les
deux seules issues étaient donc de retomber sur des bords pleine largeur
(`▀▄█▄▀`) ou d'accepter que deux cellules sortent de la police.

**La seconde a été retenue, sur demande explicite, et c'est la première fois de
tout le dossier.** Trois fois auparavant — les blocs de hauteur de la jauge, les
blocs partiels à seize crans, les dix quadrants du logo d'origine — l'absence
dans Consolas avait valeur d'interdit. Elle garde ici valeur d'avertissement, et
le rendu se juge à l'écran comme le reste.

Ce qui change, c'est la façon dont le test le porte. `REPLIS_ASSUMES` recense
nommément les glyphes hors police, si bien que la garantie ne disparaît pas :
elle se déplace de « aucun glyphe hors police » vers « aucun glyphe hors police
**qui n'ait été décidé** ». Le test vérifie en plus que chaque exception sert
encore — une liste d'exceptions qui survit à son motif est un fourre-tout où le
prochain oubli passerait inaperçu.

> C'est la forme générale d'une bonne dérogation : ne pas supprimer le contrôle,
> mais lui faire nommer ce qu'il laisse passer.

### Le bitmap était le mauvais outil — le soir du 26/08

Sixième passage, et le seul qui ne soit pas un redessin : `▝▄▀▄▘` devient
`(V)°°(V)`. Cinq itérations avaient négocié des pixels de coin sur une marque
qu'on ne regarde que du coin de l'œil ; la sixième change de registre.

Le raisonnement tient en une phrase. **Une cellule de terminal ne porte que deux
couleurs et une forme fixe** : à cette résolution, la masse qui rend le crabe
reconnaissable est aussi ce qui le fait peser sur la ligne, et chaque dessin
gagnait en fidélité ce qu'il perdait en discrétion. L'émoticône échappe à
l'arbitrage — elle ne dessine pas le crabe, elle le désigne, et confie le reste à
la lecture. Les caractères portent déjà des formes : une parenthèse *est* une
pince, et il suffisait de les assembler.

Trois conséquences, dont deux n'étaient pas cherchées.

**a. Plus aucun repli de police.** `(`, `V` et `)` sont de l'ASCII, `°` (U+00B0)
est dans Consolas. La dérogation du soir précédent — `REPLIS_ASSUMES`, et la
garantie déplacée vers « aucun glyphe hors police qui n'ait été décidé » —
disparaît d'elle-même : le test `les_cellules_du_logo_sont_dans_consolas` retrouve
sa forme absolue, sans liste d'exceptions. C'est le meilleur sort qu'on puisse
faire à une dérogation, et il n'était pas prévu.

**b. Huit colonnes, et plus légères que cinq.** La largeur augmente de trois
colonnes, la charge d'encre s'effondre : le glyphe le plus chargé est un `V`, là
où `▝▄▀▄▘` posait trois demi-blocs. En corps 8 *light*, c'est l'encre qui décide
du poids d'une marque, pas la largeur — ce que la table d'arbitrage de la
compacité, raisonnant en colonnes, ne pouvait pas exprimer.

**c. Le fond noir revient, et ce n'est pas un aller-retour.** `clawd_background`
avait été abandonné le matin parce qu'un noir en dur ne suit rien. L'objection
portait sur les **creux** d'un dessin en demi-blocs — yeux, coins, dessous des
pinces n'étaient que du fond laissé visible. `(V)°°(V)` n'a pas de creux : ce
sont des traits posés sur un plan. Le logo prend donc une pastille comme le
segment d'emplacement, avec la même convention — fond profond de la teinte, texte
clair de la même teinte, le fond épousant exactement le texte.

> **L'arbitrage de contraste diffère de celui de la pastille d'emplacement**,
> parce qu'une des deux teintes est imposée. `CODE_PASTILLE` choisit ses deux
> couleurs et atteint 10:1 ; ici le texte est `rgb(215,119,87)`, couleur de
> marque, et ne bouge pas. Contre un noir absolu il plafonnerait à 6,7:1. Le fond
> retenu, `rgb(48,14,0)`, en rend 5,6:1 — au-dessus du seuil AA — et garde au
> plan sa chaleur. Le point général : **quand une teinte est imposée par la
> fidélité, le contraste se borne avant de se régler**, et il vaut mieux le
> chiffrer que le constater à l'écran.

| Étape | Résultat |
|---|---|
| Portage Rust : 2 constantes réécrites, 1 fonction | `LOGO`, `CODE_LOGO`, `prefixer_logo_selon` |
| Tests du crate | 40, inchangés — un renommé, deux réécrits |
| Transcription dans l'oracle PowerShell | **aucune divergence sur 87 cas** |
| Longueur de la ligne | 6 → 11 colonnes de marque, puis 9 après le retrait des liserés |

> Le premier passage du harnais a signalé deux divergences, toutes deux sur
> `rythme epuisement`, à une minute d'écart. C'est la course que le harnais
> documente lui-même — il projette une heure à partir de l'instant présent et
> lance les deux implémentations l'une après l'autre. Relancé, il rend
> « aucune divergence ». Le message d'erreur qui nomme sa propre fausse alerte
> vaut mieux que n'importe quelle note de dossier.

### Les liserés : quand un réglage n'a que deux crans

Dernière retouche du jour, et la première à porter sur **les deux** pastilles à
la fois : retirer l'espace intérieur qui les aérait. La demande était de les
« diminuer » ; la grille de terminal n'offrait pas ce choix.

C'est le point qui mérite d'être noté. Un liseré, dans une interface graphique,
se règle en fractions de cadratin — 2 px, 4 px, 0,3 em. Dans un terminal, il
occupe **une cellule pleine ou rien** : il n'y a pas de demi-colonne, et un
espace fin (U+2009) ou une chevelure (U+200A) y sont rendus à pleine chasse comme
tout le reste. Diminuer et supprimer sont donc le même geste, et il fallait le
dire avant d'agir plutôt que de rendre un demi-résultat.

| | Avant | Après |
|---|---|---|
| Pastille du logo | ` (V)°°(V) ` | `(V)°°(V)` |
| Pastille de l'emplacement | ` PY_xl ` | `PY_xl` |
| Ligne de référence | 57 colonnes | **53 colonnes** |

Ce qui rend le retrait tenable tient à deux choses déjà au dossier. Le fond
**est** la mise en valeur : il porte la limite du fragment à lui seul, et le
liseré n'y ajoutait qu'un confort. Et la pastille d'emplacement est encadrée de
`│` depuis le §12 — décision prise contre une proposition d'économie de six
colonnes, au motif que le fond ne sépare que pour l'œil qui voit les couleurs.
Cette barre, gardée pour `NO_COLOR`, paie ici une seconde fois : le segment reste
détaché de ses voisins sans rien devoir aux espaces.

> **Suite, le soir même : cette barre est tombée à son tour — voir §16.** La
> conclusion de ce paragraphe lui survit, le fond suffisant aux deux rôles ;
> c'est même en le constatant ici qu'on a vu qu'elle était de trop.

> **Effet de bord sur le code.** `pastiller` / `Add-Pastille` se réduit à
> `colorer` / `Add-Couleur` sous un autre nom — la branche `NO_COLOR` n'a plus
> rien à défaire. La fonction est conservée quand même : le nom porte
> l'intention, que `CODE_PASTILLE` seul ne porterait pas, et les deux appelants
> continuent de dire ce qu'ils font. Une indirection qui ne fait plus rien mais
> qui nomme encore quelque chose n'est pas un déchet.
>
> Le test a été renommé en conséquence — `pastille_epouse_le_texte_et_se_retire_sans_couleur` —
> et sa première assertion interdit désormais le retour des liserés. Sans elle,
> plus rien dans le code ne rappellerait qu'ils ont existé ni pourquoi ils sont
> partis.

### La casse d'un glyphe est une propriété d'affichage

Détail minuscule, même leçon que le §13. `(v)°°(v)` a été déposé avant
`(V)°°(V)` : en bas de casse, le `v` s'assied sur la ligne de base, sous l'axe
des parenthèses, et la pince paraît décrochée. La capitale monte à hauteur de
parenthèse, et les trois traits se referment sur le même axe.

Rien dans le code ne pouvait le dire. Comme le noir de `clawd_background`, comme
les sept crans de jauge, comme le damier : **une propriété d'affichage se
regarde**. La différence, ici, est qu'elle tient à la métrique d'une police —
hauteur d'x contre hauteur de capitale — et qu'elle aurait aussi bien pu se
mesurer dans `consola.ttf`. On ne l'a pas fait, et c'est l'œil qui a tranché.

### Ce qui reste ouvert

- **Les trois autres poses.** Le composant en porte quatre ; la ligne n'en sert
  qu'une. Une pose qui suivrait l'état de la session — `arms-up` sur un palier
  critique, par exemple — est tentante et probablement une mauvaise idée : elle
  ferait du logo un segment, c'est-à-dire exactement ce qu'il n'est pas.
- **La pastille en tête de ligne**, écartée au §14, est finalement arrivée par
  l'autre bout — non comme ancre pour l'œil, mais comme plan du logo. La ligne
  pose désormais deux fonds, aux deux extrémités de la hiérarchie : ce qui ne dit
  rien de la session, et le seul segment qui vaille d'être ancré.
- **Le vocabulaire de Consolas est plus large qu'on ne le croyait.** Le relevé du
  soir a ajouté aux huit blocs connus les filets arrondis `╭╮╯╰`, les diagonales
  `╱╲`, les demi-filets épais `╸╹╺╻`, les formes `●○▪▬▲`, et `∩`. De quoi
  dessiner à trait fin, ce que les demi-blocs interdisaient — piste ouverte si le
  bitmap devait revenir.
- **`planche-logo.ps1`** compose les variantes écartées — dont la reprise
  littérale des glyphes d'origine et les propositions à trait fin — avec les
  vraies séquences SGR. Le logo reste rejugeable sans rien recompiler.

## 16. La barre de groupe retirée : une décision renversée en une journée — 26/08/2026, au soir

Retouche suivante, le même soir, et la seule de la série à **défaire** une
décision consignée plutôt qu'à en prendre une nouvelle. La barre `│` qui
encadrait la pastille de l'emplacement est tombée ; les trois groupes se
joignent désormais par un espace simple.

| Étape | Résultat |
|---|---|
| Portage Rust | 1 constante réduite à l'espace, 1 supprimée (`CODE_SEPARATEUR_GROUPE`), 1 fonction retirée (`separateur_groupe`) |
| Tests du crate | 40, inchangés — deux renommés, un verrou de non-retour ajouté |
| Transcription dans l'oracle PowerShell | **aucune divergence sur 87 cas**, au premier essai |
| Longueur de la ligne | 86 → **82 colonnes** au calme |

### Ce qui a changé n'est pas l'argument, c'est son prix

Le §12 avait examiné exactement cette proposition et l'avait **écartée**, au motif
qu'elle est vraie en couleur et fausse sous `NO_COLOR` : le fond ne sépare que
pour l'œil qui voit les couleurs, la barre sépare pour tout le monde. Cet
argument n'a pas été réfuté, il tient toujours mot pour mot.

Ce qui a bougé, c'est ce qu'il fait payer, et à qui. La barre se prélevait sur
**tous** les affichages pour tenir le seul où la hiérarchie des teintes est déjà
éteinte, où le logo disparaît entièrement, et que personne ne regarde :
`NO_COLOR` est l'environnement des sous-processus lancés par Claude Code, pas
celui de la ligne affichée. Un dispositif qui coûte sur le cas nominal pour
servir un cas dégradé est un mauvais échange dès qu'il existe une autre façon de
tenir le cas nominal — ici, le fond de la pastille.

**La leçon est dans l'arithmétique du milieu.** Trois groupes ne font que deux
jonctions, et le groupe de l'emplacement est des deux côtés : la barre n'a jamais
eu d'autre travail que de séparer la pastille de ses voisins. Une règle générale
— « les groupes se séparent par une barre » — qui, sur une ligne à trois groupes,
ne s'applique qu'à un seul cas particulier déjà couvert autrement. C'est ce que
le §12 n'avait pas vu : il raisonnait sur la *règle*, pas sur ses *occurrences*.

### L'ordre des deux retraits n'est pas indifférent

La barre avait servi, quelques heures plus tôt, à justifier le retrait des
liserés intérieurs de la pastille (§15) : le fragment restait détaché sans
rien devoir aux espaces. C'est en constatant que le fond seul suffisait à borner
le segment qu'on a vu qu'il suffisait aussi à le détacher — les deux rôles sont
le même trait de couleur, lu deux fois.

Une justification tirée d'un dispositif est donc à relire quand ce dispositif
disparaît. Ici la conclusion survit à sa prémisse, mais rien ne le garantissait :
c'est le genre de dette qu'un dossier de décisions doit rendre visible, et c'est
la raison d'être des trois entrées de commentaire laissées dans le code — deux
constantes et `Add-Pastille` / `pastiller` renvoient les unes aux autres.

> **Le verrou de non-retour.** `groupes_joints_par_l_espace_et_segments_par_le_point`
> vérifie désormais que la ligne assemblée ne contient **aucun** `│`. C'est la
> même mesure que la première assertion de
> `pastille_epouse_le_texte_et_se_retire_sans_couleur`, prise pour la même
> raison : sans elle, rien dans le code ne rappellerait qu'une barre a existé ni
> pourquoi elle est partie — et la prochaine relecture de la hiérarchie des
> séparateurs la réintroduirait comme une évidence.

### Ce qui reste ouvert

- **La ligne sous `NO_COLOR` n'a plus de rang de groupe.** Elle garde le `·` de
  ses segments et rien au-dessus. Si cet affichage devait un jour être regardé —
  un journal, un export, un terminal sans couleur — le rang supérieur y serait à
  rétablir, et ce serait un comportement conditionné par `NO_COLOR`, c'est-à-dire
  le seul endroit de la ligne où l'absence de couleur *ajouterait* du texte au
  lieu d'en retirer. Écarté ici faute de cas d'usage, pas faute de faisabilité.
- **Deux rangs sur trois s'écrivent maintenant par un espace.** Le rang du milieu
  est le seul à porter un glyphe. C'est tenable tant que la pastille est là pour
  distinguer les deux blancs ; un second segment mis en pastille, ou la pastille
  déplacée ailleurs sur la ligne, rendrait la question entière à réexaminer.

> **Suite immédiate.** Le point ci-dessus a été tranché dans l'heure : un second
> segment *a* été mis en pastille — l'abonnement, en tête de ligne. Voir §17.

## 17. Le logo cède sa pastille à l'abonnement — 26/08/2026, au soir

Le crabe `(V)°°(V)` posé en tête de ligne quelques heures plus tôt (§15) est
retiré. **L'abonnement prend sa place, sa pastille et sa teinte** — mêmes
caractéristiques, autre contenu.

| Étape | Résultat |
|---|---|
| Portage Rust | `LOGO` supprimée, `CODE_LOGO` renommée `CODE_PASTILLE_ABONNEMENT`, `prefixer_logo` remplacée par `pastiller_abonnement` |
| Tests du crate | 38, contre 40 — trois tests du logo retirés, un test des deux pastilles ajouté |
| Transcription dans l'oracle PowerShell | **aucune divergence sur 87 cas**, au premier essai |
| Longueur de la ligne | 82 → **71 colonnes** au calme |

Onze colonnes, soit près d'un septième de la ligne : huit pour le logo, une pour
son espace, deux pour le `·` que l'abonnement traînait derrière lui et qui est
devenu la jonction de son propre groupe.

### Une marque qui ne dit rien contre un mot qui dit quelque chose

Le §15 avait pesé le logo à l'aune de sa **discrétion** — six dessins, une
émoticône, la charge d'encre plutôt que la largeur — sans jamais poser la
question de ce qu'il *apportait*. C'était pourtant la seule qui décide : à cet
endroit de la ligne, huit colonnes disent « ceci est Claude Code » à un lecteur
qui vient de taper une commande dans Claude Code.

L'abonnement, lui, tient en trois colonnes et dit sous quel régime la session
tourne — c'est-à-dire ce qui gouverne la taille des fenêtres de limitation
affichées à l'autre bout de la même ligne. Il était déjà là, atténué au rang du
chrome ; la pastille le fait ressortir sans qu'aucune colonne s'ajoute.

**La teinte survit à ce qu'elle peignait.** `clawd_body`, `rgb(215,119,87)`, est
la couleur du produit dans les six thèmes en couleurs vraies du bundle — pas une
couleur de dessin. La reprendre pour le mot qui nomme l'abonnement du produit est
plus fidèle que de l'avoir donnée à un crabe : elle est maintenant à l'endroit de
la ligne où le produit se nomme.

### Ce que l'échange coûte : la ligne dégradée n'est plus signée

Le logo était posé au point de sortie unique — `terminer` côté Rust,
`Write-LigneStatut` côté script — et paraissait donc sur **tous** les chemins,
replis compris. C'était son seul argument fonctionnel, et il tombe : sur un
payload que le programme n'a pas su lire, la ligne se réduit désormais au nom du
modèle, sans rien qui la signe.

Le remplacer à l'identique aurait demandé de lire le disque sur un chemin
d'erreur pour afficher `Pro Claude`, ce qui n'aurait signé qu'au prix d'une
ligne absurde. L'abonnement est un segment, il s'assemble et se retire comme tel.

> **Il a pris son propre groupe.** Quatre groupes au lieu de trois, et la
> mécanique existante lui donne gratuitement le bon comportement quand la
> configuration est illisible : le groupe vide emporte son séparateur, la ligne
> commence par la version, pas par un espace orphelin. C'est le même mécanisme
> qui avait effacé la barre d'un groupe absent — écrit une fois, il sert la
> deuxième sans qu'on y touche.
>
> Le groupe propre a aussi retiré le `·` entre l'abonnement et la version : un
> fond n'a pas besoin d'un point pour qu'on voie où il s'arrête. C'est
> l'argument du §16, appliqué une deuxième fois le même soir — et cette fois
> sans même avoir à en discuter, la démonstration ayant été faite une heure plus
> tôt.

### Ce que la frontière `NO_COLOR` a tranché toute seule

Le logo disparaissait entièrement sous `NO_COLOR` : privé de couleur, `(V)°°(V)`
n'est pas une version sobre du crabe, c'est huit caractères de ponctuation en
tête d'une ligne dont ils ne disent rien.

L'abonnement, lui, **garde son texte**. La règle était déjà écrite pour la
pastille d'emplacement — un fragment qui informe survit à la perte de sa teinte,
une marque que la couleur constitue s'en va avec elle — et elle a suffi à
trancher sans qu'aucune décision nouvelle soit nécessaire. C'est le genre de
frontière qui prouve sa valeur quand un cas non prévu s'y range de lui-même.

### Ce qui reste ouvert

- **`planche-logo.ps1` ne sert plus la ligne.** Le script est conservé : il
  compose les six variantes du crabe avec les vraies séquences SGR, et reste le
  seul moyen de rejuger le dessin si la question revenait. Un outil qui documente
  une décision abandonnée n'est pas un déchet tant qu'il dit pourquoi.
- **Deux pastilles, et plus de rang libre.** La ligne pose désormais ses deux
  fonds sur ses deux informations les plus stables. Un troisième les banaliserait
  toutes : le fond n'est une mise en valeur que tant qu'il est rare.

## 18. La fenêtre hebdomadaire suit le modèle — 03/09/2026

Demande d'origine : « le groupe de la consommation des crédits à 7 jours doit
s'adapter au modèle, car si Fable est sélectionné les crédits sont épuisés plus
vite que pour les autres modèles ». Première évolution de **logique** depuis le
portage qui touche à la forme du cache, donc à tous les cas du harnais.

| Étape | Résultat |
|---|---|
| Ce que le payload dit | capture sur 2.1.259, en session Fable : `five_hour`, `seven_day`, rien d'autre |
| Ce que le binaire sait | `seven_day_opus`, `seven_day_sonnet`, `seven_day_overage_included` — la « Fable limit » de `/usage` —, non transmis à la ligne |
| Ce que la grille tarifaire dit | Fable 5.1 à 10 $ / 50 $, Opus 5 à 5 $ / 25 $ : facteur **2**, identique en entrée et en sortie |
| Portage Rust | module `modele.rs` neuf ; `Fenetre::selon_modele` ; `calculer_ancrage` prend la famille ; `observe_modele` dans le cache |
| Tests du crate | 38 → **49** |
| Transcription dans l'oracle PowerShell | quatre fonctions et un champ ; **aucune divergence sur 102 cas**, au premier essai |
| Harnais | 87 → **102 cas** |
| Seuils hebdomadaires sous Fable | 75 / 85 → **50 / 70** |

### Pourquoi un facteur, et non une lecture

La première question n'était pas « comment adapter » mais « qu'est-ce que
Claude Code envoie ». La capture a tranché en une seconde, comme au §12 : le
payload ne porte aucune fenêtre propre au modèle. Le scan du binaire a ensuite
montré ce qui existe **sans être transmis** — trois fenêtres par modèle, dont
la « Fable limit » qui fait basculer Fable sur les crédits payants une fois
l'usage inclus de la semaine consommé. Le constructeur du payload de la ligne
de statut n'en prend que `five_hour`, `seven_day` et un `spend_limit` de
passerelle.

Restait à lire le facteur quelque part de fiable. La grille tarifaire du jour
le donne sans hypothèse : le budget hebdomadaire se vide au prix du jeton, et
Fable 5.1 vaut le double d'Opus 5, en entrée comme en sortie. Les autres
familles y figurent pour que la table soit complète et vérifiable, non pour
agir — voir ci-dessous.

### Ce que le facteur change, et ce qu'il laisse

Deux effets, sur la seule fenêtre de 7 jours :

- **les seuils descendent**, par la réserve : les points qui séparent le seuil
  du plafond sont multipliés par le facteur, si bien que l'alerte laisse le même
  temps de réaction dans une monnaie qui vaut deux fois moins. Un facteur
  inférieur à 1 ne relève rien — le compteur est partagé, et une session Sonnet
  ne rend pas moins cher ce que Fable a déjà consommé la même semaine ;
- **le rythme se réancre** quand la famille change, la pente mesurée sous l'une
  ne disant rien de l'autre. C'est la famille qui compte, pas la valeur du
  facteur : Opus 4.8 et Opus 5 partagent la leur. Le cache porte la famille de
  l'ancrage, et le réancrage demande les deux familles — un cache antérieur au
  champ n'est pas un changement, ce qui a préservé l'ancrage de la semaine en
  cours au premier lancement.

Ce qu'il laisse : la fenêtre de 5 heures, qui se réancre d'elle-même à chaque
remise à zéro et qu'un silence de cinquante minutes à chaque bascule pénaliserait
plus qu'une pente datée de quelques heures ; et la projection hebdomadaire
elle-même, qui se tait après une bascule le temps que `RATIO_MAX_EXTRAPOLATION`
soit de nouveau satisfait — jusqu'à une journée en début de fenêtre. C'est le
prix du principe « mesurée, jamais déduite » ; le scalaire aurait permis de
projeter aussitôt en multipliant l'ancienne pente, et c'est exactement le genre
de projection affirmée que le §6 de la première version du script refusait.

### La règle du §6, une sixième fois — et par le cache

Le champ `observe_modele` s'écrit à chaque lancement qui mémorise une fenêtre.
Ce n'est pas un cas de plus qui diverge, c'est la forme du fichier que le
harnais compare octet pour octet : ne l'écrire que côté Rust n'aurait pas
entamé l'oracle, il l'aurait supprimé. Le critère du 26/08/2026 — ce qui ferait
diverger le harnais entier se porte — a donc joué une fois de plus, pour de la
logique cette fois, et la transcription a tenu au premier essai.

Une différence avec les fois précédentes : la sérialisation d'une **chaîne**
dans le cache, là où il n'y avait que des entiers et des `null`. `Display`
d'une `Value::String` côté Rust et `ConvertTo-Json` côté oracle produisent la
même chaîne JSON pour ces valeurs ASCII ; une famille accentuée aurait demandé
de vérifier l'échappement des deux côtés, et il n'y en a pas.

### Ce qui reste ouvert

- **La « Fable limit » n'est pas affichée.** Elle existe, Claude Code la
  connaît, et c'est elle qui décide du passage aux crédits payants. Si une
  version transmettait `seven_day_overage_included` ou `model_scoped` à la
  ligne de statut, un segment dédié vaudrait mieux que le facteur pour cette
  part-là. La capture après chaque mise à jour est le geste qui le dira.
- **Les facteurs suivent une grille tarifaire.** Ils sont datés du 03/09/2026 et
  se revérifient à chaque changement de prix ; la table est au même endroit que
  les seuils, en tête des réglages.
- **La fenêtre de 5 heures pourrait suivre.** Un drapeau sur son descripteur
  suffit, si l'usage montre que la pente datée gêne plus que le silence.

## 19. La fenêtre du modèle, relevée par `/usage` — 03/09/2026, au soir

Le §18 déployé, l'utilisateur ouvre `/usage` : 28 % pour Fable sur la semaine,
quand la ligne dit 14. « Le dernier groupe ne fonctionne pas puisqu'il diffère
de la commande `/usage`. » Le §18 avait conclu que la fenêtre propre à Fable
existait sans être transmise ; il manquait de regarder **où** Claude Code
l'écrit.

| Étape | Résultat |
|---|---|
| Où est le 28 % | `~/.claude.json`, `cachedUsageUtilization.utilization.limits[]`, entrée `weekly_scoped` dont `scope.model.display_name` vaut « Fable » |
| Fraîcheur, dans le bundle | réécrit au plus toutes les 5 minutes (`Qso`), relu si moins d'une heure (`Xso`), à l'occasion d'un appel à `/api/oauth/usage` |
| Fraîcheur, mesurée | `fetchedAtMs` immobile pendant dix minutes d'activité soutenue : le relevé ne bouge qu'à l'ouverture de `/usage` |
| Portage Rust | module `usage.rs` neuf ; `lire_config` partagée avec l'abonnement ; `FENETRE_MODELE` sous la clé `seven_day_modele` ; `Fenetre` scindée en `seuils_selon_modele` / `ancrage_selon_modele` ; la place « 7j » va à la plus remplie |
| Tests du crate | 49 → **55** |
| Transcription dans l'oracle PowerShell | `Get-Configuration`, `Get-FenetreModele`, `Measure-Fenetre`, `Format-Fenetre` rendant aussi le pourcentage ; **aucune divergence sur 116 cas**, au premier essai |
| Harnais | 102 → **116 cas**, sur des relevés fabriqués avec l'instant des cas |

### Le fichier était déjà ouvert

Le segment d'abonnement lit `~/.claude.json` à chaque rafraîchissement depuis
le 26/08/2026, et le §12 de ce dossier avait posé la règle : ce que le payload
tait se lit sur le disque, jamais en lançant un processus ni en ouvrant le
fichier des jetons. La fenêtre du modèle y était, à quarante lignes de
`oauthAccount`. Le scan du §18 avait trouvé les clés internes et le libellé
« Fable limit », mais un scan montre un schéma, pas un fichier ; c'est
l'exploration des clés de `~/.claude.json` qui a montré le relevé — et son
`fetchedAtMs`, qui a mis la question de la fraîcheur sur la table avant que
rien ne soit écrit.

La lecture est désormais **unique** : `lire_config` / `Get-Configuration`
analysent le fichier une fois, et les deux segments reçoivent le résultat. Le
surcoût mesuré au §12 pour l'abonnement — 1,3 ms — n'est donc pas doublé.

### La place va à la plus remplie

Deux fenêtres hebdomadaires s'appliquent à une session Fable : la globale, que
tous les modèles remplissent, et celle de Fable. La ligne n'a qu'une place
« 7j », et la donne à la **plus remplie** — celle qui bornera la session la
première, et toujours une barre de `/usage`. À égalité, celle du modèle, que
`/usage` attribue à la session. Chacune garde ses seuils : ceux d'Opus pour la
fenêtre du modèle, dont le budget est déjà celui de Fable ; les seuils abaissés
du §18 pour la globale. D'où la scission du drapeau `selon_modele` en deux —
la fenêtre du modèle suit la famille par son ancrage, pas par ses seuils.

Chaque fenêtre est mesurée sous sa propre clé de cache, celle du modèle à
l'**instant de son relevé** et non maintenant : c'est à cet instant-là que le
pourcentage était vrai, et la pente comme la projection se calculent dans ce
repère. Le cache passe à trois entrées quand le modèle a une fenêtre propre.

> **Règle retournée le lendemain.** « La plus remplie » a tenu une nuit : au
> réveil de la semaine, elle donnait la place à une globale à 2 % contre une
> fenêtre Fable à 0 %. Voir §20.

### Une valeur ancienne, pas fausse

Le relevé ne bouge qu'à l'ouverture de `/usage`. L'afficher nu deux heures plus
tard dirait une mesure qui n'en est plus une ; ne plus l'afficher du tout
ramènerait la ligne à la fenêtre globale, et 28 deviendrait 14 sans qu'on sache
pourquoi. La ligne reprend donc la borne de Claude Code — une heure — et, au-delà,
le marqueur qu'elle réserve déjà aux valeurs de cache : `~28%`, sans rythme,
tant que la fenêtre n'a pas expiré. Le `~` dit alors qu'il est temps d'ouvrir
`/usage`. C'est la frontière du §17 appliquée une fois de plus : une valeur qui
informe survit à la perte de sa fraîcheur, à condition de la déclarer.

### La règle du §6, une septième fois

Une entrée de cache en plus, une lecture partagée avec un segment que tous les
cas exercent : ne porter que le Rust aurait supprimé l'oracle, pas entamé. Les
quinze cas du §18 ont dû, au passage, détourner leur configuration vers un
fichier sans relevé — le compte réel du poste en porte un, et il aurait donné à
Fable une fenêtre propre là où ces cas mesurent les seuils du facteur. Les
quatorze cas du relevé fabriquent leurs configurations dans le bac, sur
l'instant même des cas, pour que les remises à zéro coïncident avec celles des
caches injectés : sans cela, aucun ancrage ne tiendrait sur la fenêtre du
modèle.

Une nuance nouvelle dans la transcription : `ConvertFrom-Json` convertit la
remise à zéro ISO du relevé en `[datetime]` local, là où Rust lit la chaîne. Les
deux tombent sur la même seconde, et c'est le cache écrit — comparé octet pour
octet — qui le prouve.

### Ce qui reste ouvert

- **Le relevé ne se rafraîchit pas tout seul.** C'est la limite du dispositif,
  et elle est du côté de Claude Code : rien ne déclenche l'appel à l'API de
  consommation hors de `/usage`. Une version qui l'appellerait périodiquement,
  ou qui transmettrait `seven_day_overage_included` à la ligne, rendrait le
  `~` rare ; la capture après chaque mise à jour le dira.
- **Le facteur du §18 reste sur la fenêtre globale.** Elle se vide toujours
  deux fois plus vite sous Fable, et ses seuils restent abaissés ; mais c'est
  désormais la fenêtre du modèle qui occupe la place le plus souvent, avec ses
  seuils ordinaires. Si l'expérience montre que la globale n'est jamais la plus
  remplie, le facteur ne servira plus qu'au réancrage. — *Tranché dès le
  lendemain, autrement : voir §20.*

## 20. La fenêtre vierge, et la règle retournée — 04/09/2026

Le lendemain du §19, à la première session Fable de la semaine : « ma
statusline n'affiche pas la consommation à 7 jours correspondant à mon
modèle ! ». La ligne disait `7j ░░ 2%` ; `/usage` disait 2 % pour tous les
modèles et 0 % pour Fable.

| Étape | Résultat |
|---|---|
| Le relevé du poste | entrée `weekly_scoped` Fable : `percent: 0`, **`resets_at: null`**, `is_active: false` — relevé de 21:02, trois minutes avant la session |
| Ce qu'en faisait `fenetre_modele` | rejet : une remise à zéro illisible retirait l'entrée, et la place retombait sur la globale |
| Ce qu'en fait `/usage`, dans le bundle 2.1.259 | `Bce` retient les `weekly_scoped` dont le modèle figure dans la liste des modèles à fenêtre propre, sans regarder la date ; le composant `io` ne se retire que sur `utilization === null`, et n'écrit « Resets … » que si la date existe |
| Six minutes plus tard | Fable à 1 %, remise à zéro posée au jeudi suivant : la date nulle ne dure que jusqu'à la première requête comptabilisée |
| Second constat | la date posée, la règle du §19 donnait encore la place à la globale : 2 % contre 1 % |
| Correction 1 | une remise à zéro nulle passe, sans échéance : ni heure ni rythme, le pourcentage reste ; le cache mémorise `resets_at: null`, que `fenetre_memorisee` ignore ensuite |
| Correction 2 | la place « 7j » va à la fenêtre du modèle, sauf si la globale est **en alerte et plus remplie** — `Candidat` porte désormais `en_alerte`, `Format-Fenetre` rend `EnAlerte` |
| Tests du crate | 55 → **57** |
| Harnais | 116 → **121 cas** ; une divergence de 94 contre 95 % sur « rythme projection en alerte » au premier passage, aucune au second — une seconde d'écart entre les deux lancements, sur un cas étranger à la modification |
| Binaire | recompilé et déployé ; sur le relevé réel du poste, `7j ░░ 1%` là où l'ancien disait `7j ░░ 2%` |

### Ce que « la plus remplie » ratait

Le §19 donnait la place à la fenêtre qui bornera la session la première, et
lisait cela dans le pourcentage courant. Or les deux fenêtres ne se
remplissent pas à la même vitesse : sous Fable, la fenêtre du modèle a pris
28 points la semaine où la globale en prenait 14. Une globale à 2 % contre une
fenêtre Fable à 0 % n'est pas la plus menaçante, elle est la plus ancienne — et
la ligne, pendant une heure de travail, ne disait rien du modèle qui tourne.

La règle retournée garde ce que l'ancienne protégeait : une globale remplie
par un autre modèle plus tôt dans la semaine, au point d'alerter, reprend la
place si elle est la plus remplie. Le seuil d'alerte est celui du descripteur
effectif — 50 % sous Fable —, si bien que la globale ne peut cacher la fenêtre
du modèle qu'en ambre ou en rouge, jamais en silence.

### Le bundle, une fois de plus

Le §19 avait tiré du bundle la fraîcheur du relevé ; le §20 en tire le
comportement d'affichage, et c'est lui qui a tranché : la ligne doit afficher
ce que `/usage` affiche, et `/usage` affiche une barre à 0 % sans date. Même
méthode qu'au §18 — chercher la chaîne rare, ici `weekly_scoped` puis
`Resets `, et remonter aux fonctions qui la consomment.

### Ce qui reste ouvert

- **Le second point du §19 est tranché** : la globale n'occupe plus la place
  que lorsqu'elle alerte. Le facteur du §18 garde donc deux emplois — les
  seuils abaissés qui décident de cette alerte, et le réancrage.
- **La date nulle ne dure que quelques minutes** sur ce poste, mais elle
  revient chaque jeudi soir, et vaut pour tout modèle à fenêtre propre qui n'a
  pas encore servi dans la semaine. Un modèle que la semaine ne verrait jamais
  garderait une fenêtre vierge, et la ligne dirait 0 % — ce qui est vrai.

## 21. Revue du crate : trois fragilités fermées, et ce que l'oracle a appris — 04/09/2026, au soir

Le §20 posé, une revue du crate — performance, stabilité, robustesse — sur
les seize modules, clippy en mode pédant avec les lints de restriction, et
quatre payloads forgés contre le binaire déployé.

| Mesure | Valeur |
|---|---|
| Lancement complet, médiane sur 40 tirs | 14,3 ms — minimum 11,7, p90 17,5, maximum 24,1 |
| Travail propre, relevé par le journal | 0,4 à 4,3 ms |
| `cargo clippy` par défaut | un avertissement documentaire |
| Pédant, nursery et lints de restriction | 245, dont 95 `redundant_pub_crate` ; aucun `unwrap`, `expect` ni `panic!` hors tests |

Le verdict tient en une phrase : l'invariant du §8 tient, y compris sous des
entrées hostiles, et le coût propre du programme est négligeable devant la
création du processus. Trois fragilités réelles, toutes à faible probabilité,
ont été fermées le soir même.

### 1. La coupe par octets d'`analyser_iso`

Un décalage horaire de quatre octets non ASCII — « +aéb » — faisait trancher
`&brut[0..2]` au milieu d'un caractère. La panique était rattrapée, mais la
ligne entière se réduisait au nom du modèle — voie « panic » au journal — au
lieu du seul segment perdu. Dans la même fonction, les heures du décalage
n'étaient pas bornées, et leur produit par 3600 débordait en silence sur un
nombre à seize chiffres.

Correctif : tout ce qui n'est pas ASCII est rejeté d'entrée — ISO 8601 ne
s'écrit pas autrement, et sur une chaîne ASCII toute coupe par octets tombe
sur une frontière de caractère —, et le décalage est borné à deux chiffres
par champ. Treize chaînes adverses en test : aucune ne passe, aucune ne
panique.

### 2. Le découpage de chemin sur casse à longueur variable

`formater_repertoire` comparait deux copies mises en minuscules puis coupait
l'original à la longueur du préfixe. Toute lettre dont la casse change la
longueur en UTF-8 décalait la coupe : « Ⱥ » fait deux octets, « ⱥ » trois, et
la ligne aurait rendu « ⱥrbre\rc » ; le signe Kelvin donnait « kelvin\n\src »
sur le binaire déployé. Le préfixe se compare désormais caractère par
caractère, et l'offset est pris dans l'original.

### 3. L'alignement dans la lecture des métadonnées

`version_produit` lisait des `u16` à travers un pointeur dans un `Vec<u8>`,
dont Rust ne garantit qu'un alignement de 1. Cela fonctionnait parce que le
tas de Windows aligne à 16, mais c'était indéfini en théorie. Le bloc est
alloué en mots de 32 bits, les lectures passent par `read_unaligned`, la
chaîne est copiée mot par mot plutôt que vue en place, et chaque `unsafe` du
crate porte un commentaire `SAFETY`. Un test lit le bloc de `kernel32.dll`.

### Ce que l'oracle a appris au portage

Deux fois, la première version d'un correctif a divergé de l'oracle, et deux
fois c'est l'oracle qui avait raison — non sur le sens, mais sur ce qu'il fait
réellement, ce que seule la mesure sur le poste a montré.

- **`ConvertFrom-Json` convertit les dates ISO du payload en `[datetime]`**
  avant que `ConvertTo-Instant` ne les voie, et son analyseur lit tout
  décalage à deux chiffres par champ, « +20:00 » comme « +05:60 », quand
  `DateTimeOffset.TryParse` s'arrête à ±14:00. La borne du portage a suivi :
  deux chiffres, pas quatorze heures. Le même convertisseur lit une date
  **sans décalage en heure locale**, là où `analyser_iso` suppose UTC : aucun
  cas ne l'exerce, le payload portant toujours un décalage, mais c'est une
  divergence latente à connaître.
- **`OrdinalIgnoreCase` écarte les correspondances non réversibles** : le s
  long « ſ », le signe Kelvin, le İ et le ı turcs, le signe Ohm et le « ẞ »
  n'y valent pas leur homologue, quand « Ⱥ » et « ⱥ » se valent. Ni la
  minuscule seule ni la majuscule seule de Rust ne reproduisent cette table ;
  l'exiger des deux — `meme_lettre` — rend les neuf réponses relevées.

| Étape | Résultat |
|---|---|
| Tests du crate | 57 → **63** |
| Harnais | 121 → **126 cas**, aucune divergence |
| Payloads forgés | cinq, tous en voie « ok », code 0 |
| Binaire | recompilé et déployé |

### Les cinq points restants, fermés dans la foulée

Tous mineurs, aucun ne touchant à l'invariant — et fermés le soir même, sur
demande.

- **Pourcentages hors bornes** : `convertir_pourcent` et `ConvertTo-Pourcent`
  ramènent la valeur entre 0 et 100 après l'arrondi, le cast `[int]` restant
  devant pour qu'une valeur hors de sa plage soit un rejet et non un plafond.
  « 150 » vaut désormais un plafond au palier critique, « −5 » un zéro, et la
  projection ne peut plus tomber dans le passé. Deux cas de harnais, ligne et
  cache, portés des deux côtés.
- **Diagnostics sous filet** : `terminer` enveloppe le journal et la capture
  chacun dans un `catch_unwind`. L'invariant « toujours le code 0 » ne repose
  plus sur leur seule discipline d'écriture.
- **Panique retenue** : le hook note lieu et message dans `journal.rs`, et la
  ligne de journal porte un champ `panique`, échappé en JSON, sur le seul tir
  qui a paniqué. `analyser-journal.ps1` l'affiche dans son verdict et dans la
  fenêtre autour d'un clignotement. La coupe fautive du point 1 se serait lue
  dans le journal au lieu de demander une relecture du code.
- **Entrée standard en octets** : lue par `read_to_end` et convertie avec
  perte pour l'analyse ; la capture relève les octets tels qu'ils sont arrivés,
  y compris un UTF-8 invalide — vérifié octet pour octet, témoin consommé.
- **Menus** : `NO_COLOR` lu une fois par `OnceLock`, les huit paramètres de
  `mesurer` regroupés dans une structure `Mesure` aux champs nommés,
  `reprendre` par référence, le cache sérialisé par `write!` sans chaîne
  intermédiaire.

| Étape | Résultat |
|---|---|
| Tests du crate | 63 → **67** |
| Harnais | 126 → **128 cas**, aucune divergence au second passage — le premier portait la course d'une minute sur « rythme hebdomadaire epuisement », celle que le lanceur documente |
| Payloads forgés | 150 → `100%` en rouge, −5 → `0%`, UTF-8 invalide affiché avec le caractère de remplacement et capturé à l'octet près |
| Binaire | recompilé et déployé, l'oracle redéployé avec lui |

Un piège d'outillage au passage, consigné en mémoire : l'outil d'écriture
de l'agent convertit une séquence `\u0001` du contenu en caractère de
contrôle réel, invisible à la relecture — le test d'échappement JSON du
journal a dû être écrit par PowerShell.

## 22. La dérive entre deux relevés — 05/09/2026

Le §19 avait laissé ouvert : « le relevé ne se rafraîchit pas tout seul ».
Deux jours plus tard, la capture d'écran de l'utilisateur : `7j ~17%` sur une
ligne dont tout le reste vivait, et « il faut faire un `/usage` pour qu'il
évolue ». Le `/usage` a dit 21.

| Étape | Résultat |
|---|---|
| Où en est Claude Code | 2.1.261, par capture du payload puis scan du bundle : le payload porte toujours `five_hour` et `seven_day` seuls, son bâtisseur copiant ces deux clés de l'état en mémoire et rien d'autre ; la fenêtre Fable vit pourtant dans ce même état (`unifiedWindows.seven_day_overage_included`, lue dans les en-têtes, amorcée au démarrage par `probeQuotaStatus`), mise en miroir vers le bridge de Remote Control seulement ; **un seul écrivain** du relevé (`XKn`), sur le chemin de `/api/oauth/usage`, jamais deux fois en moins de cinq minutes ; les prélectures de démarrage n'y écrivent pas, les transcripts n'en portent rien — 0 occurrence sur 7 Mo |
| Le « parfois » | la requête `get_usage` d'un client distant — app Claude, claude.ai/code — passe par le même écrivain : c'est le seul rafraîchissement sans `/usage`, et il explique qu'il arrive |
| Ce que le relevé porte de plus | la fenêtre globale, `weekly_all`, vue au **même instant** — et celle-là, le payload la donne fraîche à chaque réponse sous `rate_limits.seven_day` |
| Rapport des deux budgets | 28/14 le 03/09, 21/11 le 05/09 : deux, aux arrondis près |
| Portage Rust | `ReferenceGlobale` et `estimer_derive` dans `usage.rs` ; `formater_fenetre` prend une `estimation` qui remplace la mesure, marquée « ≈ », sans rythme ; `Mesure` la porte ; `MARQUEUR_MEMORISEE`, `MARQUEUR_ESTIMEE` et `GENRE_FENETRE_GLOBALE` dans `reglages.rs` |
| Tests du crate | 67 → **72** |
| Transcription dans l'oracle | `Get-FenetreModele` rend `reference_globale`, `Get-DeriveModele` neuf, `Format-Fenetre` et `Measure-Fenetre` prennent `-Estimation` ; **aucune divergence sur 139 cas**, au premier essai |
| Harnais | 128 → **139 cas** : quatre relevés fabriqués de plus, onze cas, et deux cas d'attribution détournés vers un relevé qui porte la globale de leur payload |
| Binaire | recompilé et déployé, l'oracle avec lui |

### Trois voies, une retenue

La première aurait été exacte : que la ligne appelle `/api/oauth/usage`
elle-même, toutes les cinq minutes. Elle demande le jeton, donc d'ouvrir le
fichier des identifiants, et de survivre à son expiration — ce que le §12
avait exclu en posant la règle « tout se lit sur le disque, jamais en lançant
un processus ni en ouvrant le fichier des jetons ». La troisième aurait été
minimale : afficher l'âge du relevé à côté du `~`, pour dire quand rouvrir
l'écran. La deuxième est celle qui répond à la plainte en restant dans la
règle, et c'est elle qui a été retenue : **estimer**.

### Ce que l'estimation vaut, et ce qu'elle ne vaut pas

Le relevé donne, au même instant, la fenêtre du modèle *F₀* et la fenêtre
globale *G₀*. Le payload donne la globale d'aujourd'hui, *G₁*. Ce que la
globale a pris depuis le relevé, la fenêtre du modèle l'a pris aussi, au
rapport des deux budgets — et ce rapport se lit dans le relevé lui-même,
*F₀ / G₀* : deux, sur les deux relevés du poste.

> *F ≈ F₀ + (F₀ / G₀) × (G₁ − G₀)*, arrondi comme tout pourcentage de la
> ligne, plafonné à cent.

Quatre conditions, faute desquelles le relevé s'affiche tel quel, `~` compris
s'il est ancien : la même fenêtre globale des deux côtés, reconnue à la
seconde de sa remise à zéro ; une avance strictement positive — un recul est
une autre fenêtre ou une correction, pas une consommation ; deux pourcentages
relevés non nuls, sans quoi le rapport n'existe pas — c'est l'état du lendemain
d'une remise à zéro, où la ligne dit 0 % tant que `/usage` n'a pas été rouvert
après la première requête du modèle.

Ce n'est plus une mesure, et le marqueur le dit : `≈32%` là où le relevé disait
28 et la globale a pris deux points. Le rythme se tait, comme sous `~` —
projeter depuis une valeur déjà estimée empilerait deux approximations. Le
cache, lui, ne sait rien de l'estimation : il mémorise le relevé et son
ancrage, et c'est sur le relevé suivant que la pente se mesurera. L'alerte,
en revanche, se juge sur l'estimation : une fenêtre estimée à 80 % passe à
l'ambre et montre sa remise à zéro, parce que c'est là qu'elle en est
probablement, et qu'attendre `/usage` pour le dire coûterait l'alerte.

La limite est assumée, et écrite : une consommation d'un autre modèle entre
deux relevés est comptée comme celle du modèle courant. Et le rapport mesuré
dans le relevé n'est le rapport des budgets que si toute la semaine a été
consommée par ce modèle ; une part prise par Opus ou Sonnet plus tôt dans la
semaine l'abaisse, et l'estimation est alors **en dessous** de la vérité — le
sens conservateur, celui qui ne fait pas lever le pied pour rien.

### La règle du §6, une huitième fois

La lecture partagée de la configuration, la mise en forme des fenêtres et
l'arbitrage de la place : trois endroits que tous les cas du relevé exercent.
Ne porter que le Rust aurait supprimé l'oracle. Transcription, donc — deux
fonctions retouchées, une neuve, deux constantes —, et **aucune divergence
sur 139 cas** au premier passage.

Le harnais a dit autre chose, avant même de tourner : deux cas de la série du
relevé auraient changé de sens. « Globale en alerte l'emporte » et « globale
plus remplie sans alerte » donnent à leur payload une globale à 60 et à 30 %
contre un relevé à 14 % ; la dérive y aurait avancé la fenêtre du modèle à
100 et à 60 %, et les deux scripts auraient été d'accord sur une ligne qui ne
mesurait plus la règle d'attribution. Le harnais compare le Rust à l'oracle,
pas à des chaînes attendues : c'est en écrivant les cas qu'on voit ce qu'ils
mesurent. Chacun a reçu un relevé fabriqué qui porte la globale de son
payload, sans avance possible.

### Ce qui reste ouvert

- **Le rapport est une borne basse** quand un autre modèle a servi dans la
  semaine. Un raffinement possible : mémoriser *ΔF / ΔG* entre deux relevés
  successifs de la même semaine, qui mesure le rapport des budgets sans cette
  hypothèse. À faire si l'écart se voit à l'écran.
- **Le lendemain d'une remise à zéro**, la fenêtre du modèle relevée à 0 % ne
  dérive pas : rien à multiplier. Un `/usage` après la première requête du
  modèle suffit, et la dérive reprend de là.
- **L'estimation disparaît d'elle-même** au relevé suivant, et le `≈` avec
  elle. Si une version de Claude Code transmet un jour la fenêtre du modèle
  dans le payload — `unifiedWindows` la porte déjà en mémoire —,
  `estimer_derive` devient du code mort ; la capture après chaque mise à jour
  le dira, comme au §19.

## 23. La capsule — 19/09/2026

La ligne tient désormais dans **deux capsules**, une par ligne : en haut la
tête (l'abonnement, brun sur la teinte de marque), l'identité (version `·`
modèle, fenêtre, effort, marqueurs, sur une ardoise `#3b414b`) et
l'emplacement (chemin relatif au projet, ancêtres en sourd, feuille et branche
en clair, sur une sauge `#274a31`) ; en bas les mesures, sur l'ardoise ou sur
la teinte du pire palier atteint — ambre sombre `#4a3d1c`, rouge sombre
`#532825`. Chaque capsule se ferme par des demi-cercles Powerline (`U+E0B6`,
`U+E0B4`), ses compartiments se joignent par le même demi-cercle, et un blanc
de liseré borde chaque texte — le soir, ces demi-disques deviendront des arcs
fins, puis la capsule sera cadrée de deux bords gris sur les rangs voisins
(« Le contour », « Le cadre »). Le crate passe en version 2.0.0 : ce n'est pas une teinte de plus, c'est un autre
contrat de sortie.

La demande était celle-là — « toute ma statusline dans une immense capsule
dessinée par Rust, avec un design résolument agréable » —, prise après un
premier atelier de quatre partis (palette accordée, palette sobre, capsules
par pastille, deux lignes) où la question de fond était déjà posée : que
deviendrait la ligne si le fond cessait d'être l'exception de deux segments
pour devenir le cadre de tout ? Une maquette interactive a fixé la forme
(préréglage « Capsule »), un chantier écrit avant toute ligne de code —
`.claude\CHANTIER-statusline-capsule.md` — en a tranché les sept questions
avec l'utilisateur, et un préalable à l'écran a levé la seule inconnue de
rendu : Windows Terminal 1.24 trace les caps Powerline lui-même, sans
`font.builtinGlyphs`, alors qu'aucune police du poste ne les porte.

### Les trois coûts que `CODE_PASTILLE` prévoyait, et ce qu'ils sont devenus

Le commentaire de la pastille d'emplacement, écrit le 21/08/2026, énumérait
trois raisons de ne jamais poser un fond courant. Elles étaient justes ; la
capsule y répond par trois règles, qui remplacent dans `sortie.rs` la règle
« fragments juxtaposés, jamais imbriqués » :

1. *« Le `ESC[0m` qui referme chaque fragment remet le fond à zéro. »* Il n'y
   a plus de `ESC[0m` dans les fragments. **Le fond appartient au
   compartiment** : posé une fois, `48;2;r;g;b`, il reste ouvert jusqu'au
   suivant ; les fragments ne touchent qu'à l'avant-plan et au gras, et ne
   referment que ce qu'ils ouvrent — `ESC[22m` après un gras, rien après une
   couleur puisque le fragment suivant pose la sienne. Chaque ligne porte
   exactement deux remises à zéro, autour du cap droit : la première rend le
   fond du terminal au cap, la seconde referme tout. Le gras du palier
   critique est sorti de `CODE_CRITIQUE` pour devenir une brique à part,
   `gras_selon`, parce qu'un `22` ne refermerait pas ce qu'un `1;` a ouvert
   au milieu d'une séquence de couleur (Q3).
2. *« Le gris et l'ambre ont été réglés pour tenir sur n'importe quel fond de
   thème. »* Ils n'ont plus à le faire : **chaque teinte est réglée pour le
   fond du compartiment** où elle sert, connu et choisi. La palette 256, dont
   c'était la raison d'être, disparaît au profit des couleurs vraies partout ;
   le texte « plein » devient une constante explicite, `CODE_TEXTE`, là où il
   sortait sans séquence dans la couleur du terminal. La ligne est devenue
   indépendante du thème et de l'acrylic ; il ne reste au thème qu'un rapport
   à influencer, celui du corps contre son fond (1,7:1 sur Dark+), qui dit si
   la forme se voit.
3. *« Un vert permanent dirait tout va bien jusque sur un `5h 95%`. »* **Le
   corps est neutre, c'est le palier qui colore.** Le compartiment des mesures
   prend le fond du pire palier qu'il porte — contexte, fenêtres, projections,
   épuisement, valeurs mémorisées ou estimées confondus, aux seuils effectifs
   du modèle. Pour cela le palier, jusque-là implicite dans chaque compteur,
   est devenu un type ordonné, `Palier`, que `formater_mesure`,
   `segment_contexte` et `segment_fenetres` remontent. La sauge de
   l'emplacement, seul aplat de couleur permanent hors la marque, hérite du
   rôle que la pastille verte tenait depuis le 21/08 : l'ancre pour l'œil sur
   un workspace multi-projets.

### Deux lignes plutôt qu'une

La première capsule était une, et faisait 134 colonnes à l'état critique. La
question posée était « que faire si la ligne dépasse la largeur du terminal »
— Claude Code la coupe, et le cap droit avec — ; la réponse de l'utilisateur
a été de la couper lui-même en deux : le régime, ce qui tourne et d'où l'on
parle en haut ; ce que la session consomme en bas. La plus large des deux
tient sous 75 colonnes sur tous les états de la maquette, et la question
disparaît. Le prix est une ligne d'écran en permanence, et une hauteur qui
passe de un à deux rangs au premier appel API d'une session dont le cache est
vide : la ligne basse **ne s'émet pas vide**, plutôt que de compter sur Claude
Code pour la retirer.

Une capsule de *n* compartiments coûte `2 + 2n` colonnes de plus que les mêmes
groupes joints par un espace : les deux caps, un liseré de chaque côté de
chaque compartiment, une jonction par frontière, moins les espaces de groupe
qu'elle remplace. La ligne haute en prend huit, la basse quatre.

### Ce que la ligne a rendu, et ce qu'elle a repris

- **Les deux pastilles**, `CODE_PASTILLE` (vert 22 sur 157, du 21/08 au
  19/09/2026) et `CODE_PASTILLE_ABONNEMENT` (teinte de marque sur `#300E00`,
  du 26/08 au 19/09/2026). La première avait été retournée le 25/08 — née
  claire sur l'hypothèse d'un terminal clair, elle criait plus fort qu'un
  `5h 95%` sur le Dark+ du poste, et sous l'acrylic une fenêtre claire passant
  derrière la confondait avec son fond ; d'où le vert profond 22 et le texte
  157, à 10:1. La seconde tenait un arbitrage de fidélité : le texte gardait
  `rgb(215,119,87)`, la couleur du produit, et le fond descendait à `#300E00`
  pour 5,6:1, « une pastille qui reste orange ». Ce que la mesure a montré le
  19/09 : ce fond faisait **1,06:1** contre le `#1E1E1E` du terminal — il
  n'existait pas à l'écran, et la pastille de tête était un mot orange posé nu.
  La capsule inverse le badge : la marque devient l'aplat, le texte descend au
  brun `#2a0d02` à 5,7:1. La couleur du produit n'a pas bougé ; elle a changé
  de rôle.
- **Le séparateur de groupe**, un espace simple depuis que la barre `│` était
  tombée le 26/08 au soir : la pastille bornait son groupe à elle seule, et la
  barre n'avait jamais séparé qu'elle de ses voisins. Le rang supérieur
  s'écrit désormais par la jonction de deux fonds — et redevient l'espace
  simple sous `NO_COLOR`, où la ligne est exactement celle de ce deuxième
  état.
- **Les liserés**, retirés des pastilles le 26/08 au soir parce que deux
  colonnes pour de l'air ne se justifiaient pas sur une grille sans cran
  intermédiaire. Ils reviennent, parce que la forme le demande : un cap
  arrondi contre une lettre se lit mal, et deux fonds qui se joignent ont
  besoin d'une cellule de respiration de chaque côté. Le budget est le même,
  la raison n'est plus la même.
- **Le point médian** descend d'un cran sous les libellés (2,7:1 sur
  l'ardoise) : l'objection du 26/08 — « deux rangs pour une seule teinte » —
  visait la barre, et rien ne dispute plus ce rang au point.
- **La jauge** se colore cellule par cellule : le `░` du vide en piste
  `#525965`, le plein dans l'encre du palier. Sous le seuil, le cran entier
  sortait en couleur de texte, et le vide était aussi lumineux que le plein.
  Ses sept crans et ses deux cellules ne bougent pas — la table de Consolas
  relevée ce jour porte aussi `▀`, `▌` et `▐`, contrairement à ce que le
  commentaire de `BLOCS_JAUGE` affirmait, mais n'ouvre pas de cran de plus
  qui soit strictement croissant.
- **Le chemin** est hiérarchisé : ancêtres et parenthèses en sourd, feuille et
  branche en clair. Trois fragments d'avant-plan ; le fond vient du
  compartiment.

### La règle du §6, une neuvième fois — et la première où elle plie

Le harnais compare le Rust à l'oracle PowerShell, pas à des chaînes attendues,
et jusqu'ici chaque évolution s'y était pliée : l'oracle suivait, ou aucun cas
n'exerçait le comportement nouveau. La capsule change la forme de toute sortie
colorée, et porter 120 Ko de PowerShell pour un repli que l'exe versionné rend
inutile n'avait pas de sens. **L'oracle est gelé** (Q2), et le dit en tête. Ce
qu'il décrit encore, c'est la sortie sous `NO_COLOR` — et là, l'égalité est
totale par construction : tout ce qui n'est que forme se retire, fonds, caps,
jonctions, liserés et saut de ligne compris, et ce qui sort est octet pour
octet la ligne d'avant. Le harnais a reçu un drapeau `-IgnorerCouleur` : **113
cas comparés, aucune divergence** ; les 26 cas colorés, sautés et comptés à
part, ont été relus un à un contre le relevé de l'exe 1.0.0 fait avant
d'écrire — deux lignes chacun (une seule pour le cas sans mesure), deux
remises à zéro par ligne, un fond par compartiment, la teinte du palier et le
gras là où on les attend. Les invariants sont figés dans les tests du crate,
au seul étage où la décision de coloration s'injecte — `sortie.rs`, 80 tests
verts —, puisque les segments lisent `NO_COLOR` dans l'environnement que
Claude Code définit pour `cargo test`.

C'est aussi la première fois que l'arbitrage joue contre la cohérence de
l'affichage : sous `NO_COLOR`, une seule ligne là où il y en a deux en
couleur. Garder deux lignes aurait coûté le filet des 113 cas pour un mode
« dégradé, jamais celui qu'on regarde ». Le filet l'emporte, et le commentaire
de `assembler_capsules` le dit.

### Les sept questions, et ce qui a été décidé

Version 2.0.0 (Q1) ; oracle gelé et `-IgnorerCouleur` (Q2) ; brique `gras` à
part (Q3) ; pas de compartiment de tête vide sans abonnement — la capsule
haute commence à l'identité, cap gauche en ardoise, et la ligne dégradée n'est
pas signée, comme depuis le 26/08 (Q4) ; deux lignes (Q5) ; la synchronisation
de `~/.claude` s'étendra au `settings.json` de Windows Terminal le jour où un
réglage de ce fichier conditionnera la ligne — ce jour n'est pas venu, P-1
n'ayant rien demandé, et la tâche est reportée hors du lot (Q6) ; la jonction
s'émet même entre deux fonds égaux, géométrie constante (Q7).

Q6 a finalement été livrée le soir même, à 21:45, sur un « ok vas-y go » posé
après la clôture : non parce qu'une clé aurait été posée — aucune ne l'a été —,
mais parce que le cadre a rendu la dépendance évidente. Depuis les arcs et les
bords, la ligne est dessinée par le terminal autant que par le binaire, et le
fichier qui règle ce terminal — police, schéma, acrylique,
`adjustIndistinguishableColors` — est celui sous lequel la palette a été jugée.
`sync-user-config.ps1` a donc une quatrième liste blanche, `$externes` : source
absolue développée à l'exécution, chemin projet, et un dossier **prérequis**
dont l'absence vaut « programme absent », entrée sautée sans écart. Première
capture à 4 208 octets, quatre chemins exercés (capture, synchro dans les deux
sens, programme absent simulé, dépôt avec sauvegarde dans un bac), le lanceur
vérifie l'entrée, les deux README la décrivent (`user-config\README.md`,
« Fichiers externes »).

### Le repli en largeur — le soir même

La capsule livrée, l'écran a dit ce que l'hypothèse taisait : Claude Code ne
coupe pas une ligne plus large que son terminal, il la **replie** sur le rang
suivant, cap compris — rien n'est perdu, rien n'est propre. La question de la
largeur, que les deux lignes semblaient avoir fermée, rouvrait pour toute
fenêtre plus étroite que le rang le plus long.

Mesurer avant d'écrire : le processus de la ligne, lancé par Claude Code à
travers le shim, **hérite de la console**. Ouvrir `CONOUT$` et lire
`srWindow` rend la largeur de la fenêtre — module `largeur.rs`, deux features
`windows-sys` de plus, aucune dépendance nouvelle — et le journal, qui porte
désormais `larg`, a répondu **120** sur quatre lancements consécutifs depuis la
session, à moins de trois millisecondes le tir. Sans console, `null`, et la
ligne garde sa disposition fixe.

D'où le repli, écrit comme lot 2 du même chantier : **rien ne se retire** pour
tenir en largeur. Chaque rangée prévue — haute, basse — est mesurée en
cellules visibles, séquences exclues, contre la largeur moins une marge
(`MARGE_LARGEUR`, une cellule) ; ce qui ne tient pas passe au rang suivant à
une frontière propre, entre compartiments d'abord, entre segments d'un
compartiment ensuite, chaque morceau gardant sa teinte. Un segment seul plus
large que la fenêtre s'émet tel quel : il s'enroulera, il n'y a plus rien à
faire. Le `Compartiment` porte pour cela ses segments et non plus un texte
joint, et `segment_fenetres` rend une fenêtre par segment — l'essai à quarante
colonnes a montré, avant correction, un compartiment des mesures qui ne savait
couper qu'entre `ctx` et le reste. Sous `NO_COLOR`, pas de repli : une ligne,
celle d'hier, et le harnais toujours à zéro divergence sur 113 cas.

Le repli s'est vu hors de Claude Code, sur un payload fictif à l'état critique
dans des consoles cachées redimensionnées par `mode con` : deux rangs à 120,
trois à 60 — l'emplacement descend —, quatre à 40 — `ctx · 5h` puis `7j` —,
six à 24, où l'emplacement de trente-neuf cellules part seul et déborde, comme
prévu. Chaque rang est une capsule entière.

### Une ligne, dépliée seulement s'il le faut — 19:29

La capture de l'utilisateur, une fois le repli livré, a confirmé que Claude
Code affiche plusieurs rangs — et montré deux rangs qui tenaient largement
côte à côte sur ses 120 colonnes. Sa demande : **une seule ligne tant qu'elle
tient**, un second rang seulement quand c'est nécessaire. La mécanique du
repli le permettait déjà : l'assemblage ne prévoit plus qu'une rangée, et
`empaqueter` la déplie seul quand la largeur l'impose. La découpe fixe « haut /
bas » du matin n'aura vécu que quelques heures : elle répondait à une largeur
que le repli mesure désormais. Sur le payload critique fictif, un rang à 140
colonnes (126 cellules), deux à 120 et 100, trois à 60, quatre à 40 ; la ligne
réelle de la session, une nonantaine de cellules, tient sur un rang.

### Le contour — le soir, après la clôture

Le chantier était clos quand trois questions sont arrivées à la file, chacune
tranchée comme P-1 : une commande de comparaison collée dans un onglet pwsh de
Windows Terminal, les teintes réelles, et la capture de l'utilisateur pour
juger (`capsule-bouts.ps1` puis `capsule-lisere.ps1`, dans le scratchpad de la
session).

1. **Des demi-cercles plutôt que des demi-disques.** Les caps et les jonctions
   passent aux jumeaux fins des mêmes glyphes Powerline, `U+E0B7` et `U+E0B5` :
   un arc, plus de remplissage. Quatre variantes montrées, la deuxième retenue
   — des arcs partout. Ce que l'arc change : l'intérieur du demi-cercle n'est
   plus la teinte du compartiment qu'on quitte mais le fond de la cellule — le
   terminal aux bouts, le compartiment suivant aux jonctions.
2. **Un liseré blanc autour de toute la pilule**, puis **l'intérieur des arcs
   dans la teinte quittée** : deux demandes qui butent sur la même limite, et il
   a fallu la poser nettement — une cellule porte un glyphe, une encre, un
   fond, rien de plus. Un disque plein cerné d'un arc d'une autre couleur ferait
   trois couleurs dans une cellule ; cela n'existe pas, et la seconde demande
   n'a pas d'autre réponse que le demi-disque du matin. Le contour, lui, se
   construit avec ce que la grille offre en plus des glyphes : les arcs en blanc
   pour les bouts ; le soulignement (SGR 4) pour le dessous, dont la couleur se
   règle à part (SGR 58) ; le surlignement (SGR 53) pour le dessus, qui n'a pas
   de couleur propre et prend l'encre du caractère qu'il coiffe. Cinq variantes
   montrées, dont une avec les demi-disques et les traits — pour voir les traits
   s'arrêter net devant la courbe. La quatrième retenue : arcs blancs, trait
   dessous, trait dessus, en connaissance du défaut — le trait du haut est
   blanc au-dessus des espaces et du texte clair, sombre au-dessus de
   « Max 5x », gris au-dessus des points. Windows Terminal 1.24 trace les trois
   sans réglage (capture de 21:04).

Ce que cela fait au code : `RVB_CONTOUR` (`236;238;241`, le blanc du texte)
dans `reglages.rs`, trois glyphes changés, et `encapsuler` qui ouvre les traits
une fois après le cap gauche — `ESC[4m ESC[58;2;…m ESC[53m` — et les laisse à
la remise à zéro du cap droit : les caps n'en portent pas, l'arc ferme déjà la
forme. Les jonctions ne prennent plus la teinte du compartiment précédent, ce
qui retire à Q7 sa seule réserve : entre deux fonds égaux, l'arc blanc se voit.
Version 2.1.0 — le contrat de sortie gagne trois SGR. 89 tests ; harnais hors
couleur à zéro divergence, la ligne `NO_COLOR` n'ayant pas bougé d'un octet.

Ce que la première capture montrait, et qui éclaire la demande de départ : sur
`Dark+` à 90 % d'acrylique, le fond composé du terminal tombait près de
l'ardoise `#3b414b` du corps — l'identité et les mesures s'y fondaient, et les
demi-disques qui les bordaient, relevés par `adjustIndistinguishableColors:
"always"`, semblaient flotter seuls, « entourés ». Le contour rend leur bord
aux compartiments sans toucher aux teintes.

Deux pièges d'outillage vus au passage, à ne pas repayer :

- **`NO_COLOR=1` traînait dans le shell de l'agent** : l'environnement de
  l'outil PowerShell persiste d'un appel à l'autre, malgré ce qu'en dit sa
  notice, et un passage du harnais l'y avait laissé. L'exe déployé semblait
  n'émettre aucun glyphe ; il émettait la ligne dégradée.
  `[Environment]::SetEnvironmentVariable('NO_COLOR', $null)` avant toute
  lecture de l'exe depuis l'agent.
- **`$PSStyle.OutputRendering = Host`** retire les séquences ANSI de la sortie
  d'un `pwsh -File` enfant lue par le parent, glyphes conservés : les octets se
  vérifient en exécutant le script dans le processus (`& $s`) et en inspectant
  la chaîne, pas la sortie redirigée.

### Le cadre — 21:13, la capture qui récuse les traits

La 2.1.0 a vécu un quart d'heure. La capture de la ligne dans Claude Code :
**pas de trait du haut**, et un trait du bas « pas assez bas ». Le premier
défaut est à Claude Code : Windows Terminal traçait les trois traits dans
l'onglet d'essai, la ligne de statut n'en montre qu'un. Le bundle de
`claude.exe`, scanné pour la règle, donne deux choses : la ligne est relayée
**rognée** — `stdout.trim().split('\n').flatMap(U => U.trim() || [])`, chaque
ligne débarrassée de ses blancs, les vides retirées — et l'analyseur SGR qu'il
embarque connaît bien `53` (`t.overline = !0`) et `58` ; ce qui se perd entre
ce modèle et l'écran n'a pas été tracé jusqu'au bout, le second défaut rendant
la question sans objet. Car le second défaut, lui, n'est à personne : un
soulignement se place à la hauteur que la police fixe — deux pixels au-dessus
du bord de la cellule pour Consolas — et aucun attribut d'une seule rangée ne
le descendra. Un cadre au ras des bords ne se dessine pas *dans* la rangée.

Il se dessine dans les rangées voisines : `▁` (huitième de bloc inférieur,
`U+2581`) sur le rang du dessus pose un trait au ras du bord haut de la
pilule, `▔` (huitième supérieur, `U+2594`) sur le rang du dessous pose l'autre
au ras du bord bas. Des glyphes de bloc, que Windows Terminal trace au bord
exact de la cellule et que Claude Code relaie comme du texte. Le prix : trois
rangs d'écran par capsule. Troisième comparaison collée (`capsule-cadre.ps1`) :
un rang d'arcs seuls pour mémoire, le cadre en blanc, le cadre en gris du
point. **Le gris** retenu — arcs compris : `RVB_CONTOUR` devient `124;132;142`.

Ce que cela fait au code : `encapsuler` rend trois lignes — bord, pilule, bord
—, les bords courant de la colonne après le cap gauche à celle avant le cap
droit (`largeur_capsule − 2`), ouverts par la séquence d'encre **avant** le
blanc qui saute le cap, sans quoi le `trim` de Claude Code décalerait le bord
d'une colonne ; plus aucun attribut de trait ; `BORD_HAUT`, `BORD_BAS` dans
`reglages.rs`. L'assemblage joint les capsules comme avant, une capsule
dépliée fait donc six rangs. `NO_COLOR` : rien, toujours la ligne d'hier.
Version 2.2.0 ; 89 tests ; harnais hors couleur à zéro divergence.

Le cadre a été vu dans Claude Code — « très joli » —, puis le mot qui clôt :
**« le binaire est satisfaisant »**, 21:4x. La question posée juste avant, s'il
n'y aurait pas intérêt à dessiner la ligne avec Dioxus dans le terminal, a eu
sa réponse sans chantier : non, parce que la ligne de statut n'est pas un
programme de terminal mais un producteur de texte que Claude Code relaie dans
ses propres cellules — aucun moteur de rendu ne passe sous les limites vues ce
soir, et le rendu terminal de Dioxus n'est de toute façon plus maintenu. Là
où de vrais dessins deviendraient possibles, ce serait hors du contrat : un
volet à côté, tenu par un processus à nous, Windows Terminal 1.22+ acceptant
les images Sixel. Idée notée, non instruite.

### Ce qui reste ouvert

- **Le surlignement perdu** entre le modèle SGR de Claude Code, qui le porte,
  et l'écran, qui ne le montre pas : cause non tracée, sans objet depuis le
  cadre par glyphes ; à reprendre seulement si un trait d'attribut redevient
  utile.

- **La capture après livraison** dit le reste : que Claude Code affiche bien
  les rangs — deux, ou davantage sur une fenêtre étroite ; `main.rs`
  l'affirmait, aucune ligne à plusieurs rangs n'avait été vue sur ce poste —,
  que la marge d'une cellule suffit à l'enroulement, que le TUI plein écran
  s'accommode d'une hauteur qui change avec la fenêtre et au premier appel, et
  que la piste `#525965` de la jauge survit à `adjustIndistinguishableColors:
  "always"`, qui peut relever un gris à 1,5:1 du fond.
- **Un cas du harnais est sensible à l'instant** : « rythme extrapolation sous
  la borne » a divergé une fois entre l'oracle et l'exe — 54 contre 55 %, un
  demi-point d'arrondi franchi dans les secondes qui séparent leurs deux
  exécutions, à minuit passé — et plus au passage suivant. Le cas est
  dimensionné pour éviter cela ; il ne l'évite pas à toute heure.
- **Le relevé du 25/08 sur `▁▂▃`** reste inexpliqué : si Windows Terminal
  trace les blocs lui-même, comme il trace les caps, ces six-là n'auraient pas
  dû changer de main entre `▃` et `▄`. Sans conséquence tant que la jauge reste
  en densité.
- **La copie `src_20260919_avant-capsule\`**, posée à côté du crate avant
  d'écrire, a été retirée à la clôture, sur le « binaire satisfaisant » de
  l'utilisateur — par lui, le garde-fou de l'agent refusant les suppressions
  récursives.
- **Les attendus colorés complets** ne vivent que dans le relevé du harnais
  (`.backups\harnais-reference-exe-1.0.0_20260919.txt` pour l'avant, le
  scratchpad de la session pour l'après). Si un jour les segments prennent
  leur décision de coloration en paramètre comme `sortie.rs`, trois de ces cas
  pourront devenir des tests de bout en bout.

## 24. La lecture bornée du bloc de version — 20/09/2026

L'après-midi du 20/09/2026, la revue de sécurité du dépôt public (chantier
`.claude\CHANTIER-statusline-securite-depot.md`) a activé CodeQL sur le
crate. Première analyse Rust, vingt-six règles, **une alerte**, niveau
*high* : `rust/access-invalid-pointer` sur `binaire.rs`, la lecture de la
langue et de la page de codes derrière le pointeur que `VerQueryValueW`
rend — « This operation dereferences a pointer that may be invalid. »

### Ce que l'analyseur voit, et ce qui est vrai

Le pointeur naît nul (`null_mut()`), part dans une fonction opaque par
`&mut`, revient, et se fait déréférencer après trois gardes — `trouve != 0`,
`n >= 4`, `!trad.is_null()`. L'analyseur ne connaît pas le contrat de
l'API, qui désigne toujours l'intérieur du bloc qu'on lui a passé ; il ne
peut donc pas prouver la validité, et il le dit. Le code était sain, le
commentaire `SAFETY` du §21.3 disait pourquoi ; l'alerte était, au sens
strict, un faux positif. La seconde lecture du même genre — la chaîne
`ProductVersion`, copiée mot par mot derrière `valeur` — n'a pas été
signalée, sans raison visible : même origine, même forme.

### Fermer par le code plutôt qu'écarter

L'utilisateur a choisi de durcir plutôt que de rejeter l'alerte, et de le
livrer aussitôt. La règle nouvelle : **le pointeur rendu par l'API n'est
qu'une adresse**. `decalage_dans_bloc` la compare à celle du `Vec<u32>` et
en fait un décalage en octets, `None` si elle tombe hors du bloc — ce que
l'API ne fait pas, mais rien n'en dépend plus. `octet_a` prend l'octet dans
la représentation mémoire du mot de 32 bits (`to_ne_bytes`), `mot_a`
assemble deux octets en `u16` (`from_ne_bytes`), à cheval sur deux mots
s'il le faut, chaque accès borné par `get`. La chaîne se copie par
`map_while` : une longueur qui dépasserait le bloc tronque, elle ne lit
pas au-delà. Plus aucun `unsafe` à la lecture — il n'en reste que sur les
trois appels Win32, incompressibles. L'alignement du §21.3 est conservé (le
bloc reste en `u32`) ; ce qui change, c'est que plus rien n'est lu à
travers un pointeur que le crate n'a pas fabriqué lui-même.

Deux tests neufs : les lectures bornées sur un bloc de deux mots (octets à
tout décalage, mot à cheval, `None` au débordement, `usize::MAX`, bloc
vide), et le décalage d'adresses fabriquées par arithmétique enveloppante
(dans le bloc, premier octet après, un octet avant, nul) — jamais
déréférencées. Le test de `kernel32.dll` du §21.3 couvre l'aller-retour
réel. 91 tests, clippy `-D warnings` et `fmt` propres.

### Version 2.2.1

Première Release publiée sous le régime posé le jour même : commit et tag
signés, Release **immuable**, `statusline.exe.sha256` à côté de
l'exécutable et empreinte dans les notes. Livrée par `deploy-statusline.bat`
sans drapeau — incrément patch *tag-aware*, `compiler-statusline.bat /test`,
harnais, reflet, push, Release. Le comportement visible ne change pas : même
chaîne de version lue, même segment.

## 25. Le segment de version retiré, le chemin replié sur sa feuille — 23/09/2026

Deux demandes de l'utilisateur, le même message : ne plus voir le numéro de
version, et un chemin long affiché `…\feuille` plutôt que
`racine\…\feuille`.

### Ce qui part

`segment_version`, `version_decalee` et leur test ; le module `binaire.rs`
entier, avec la lecture bornée du §24 ; `PREFIXE_VERSION` et
`CHEMIN_BINAIRE_RELATIF` dans `reglages.rs` ; la variable
`CLAUDE_STATUSLINE_BINAIRE`. Le compartiment d'identité ne porte plus que le
segment de modèle. Le signal d'une mise à jour en attente — la teinte cyan
posée le 26/08/2026 quand binaire et session divergent — part avec le
segment : la question n'a pas été posée à part, la demande visant le numéro
lui-même. Les features `windows-sys` restent : `Win32_Storage_FileSystem`
sert aussi `CreateFileW`, par lequel la sonde de largeur ouvre `CONOUT$`.

### Le repli

`compresser_chemin` rend `["…", feuille]` au-delà de
`PROFONDEUR_MAX_CHEMIN`. L'ellipse reste le caractère `…`, une colonne, comme
avant ; elle tombe parmi les ancêtres, donc en sourd, la feuille en clair.

Le seuil a d'abord été laissé à trois, et a baissé à **deux** dans l'heure :
l'utilisateur, placé dans `.claude\user-config`, a vu
`PY_xl\.claude\user-config` en entier et l'a récusé sur capture (« Aïe »).
Trois segments font donc déjà un chemin long : `PY_xl` et `PY_xl\.claude`
restent entiers, `PY_xl\.claude\user-config` devient `…\user-config` et
`PY_xl\PyScripts2\_genrsu_\rust\coeur` `…\coeur`. Les tests
`l_emplacement_distingue_la_feuille_des_ancetres` et
`repertoire_relatif_au_projet` fixent la forme : deux segments entiers, trois
repliés.

### L'oracle, dégelé le temps de deux retouches

Les deux changements touchent la sortie sous `NO_COLOR` — la version sur
**tous** les cas. C'est le critère du README (« Ce qui reste du script
PowerShell ») : ce qui ferait diverger le harnais entier se porte. Les deux
retouches sont donc passées dans `statusline.ps1` à l'identique
(`Get-SegmentVersion`, `Get-VersionBinaire`, `Find-Binaire` supprimés,
`Compress-Chemin` réécrit), et son en-tête le dit. Le harnais perd ses quatre
cas `binaire …` et le détournement de `CLAUDE_STATUSLINE_BINAIRE` : 135 cas,
**109 comparés, aucune divergence**, 26 colorés sautés.

86 tests, clippy `-D warnings` et `fmt` propres. Version **2.3.0** —
changement visible, comme 2.1.0 et 2.2.0. Déployée sur le poste par
`compiler-statusline.bat /test`, **pas publiée** : copie horodatée des sources
dans `.backups\20260923-153622_avant-sans-version\`.

### Les documents du dépôt public

Repris avant la publication : `README.md` (exemples, tableau des sources,
quinze modules, État), `SECURITY.md` (plus de lecture des métadonnées de
`claude.exe` ; la fraîcheur du binaire se juge à son empreinte), `guide.html`
(section de la version remplacée par une phrase, légendes, table des teintes
et du contrat, exemple de payload) et `guide\generer-guide.ps1` (cas
`version-ecart`, faux binaire et `CLAUDE_STATUSLINE_BINAIRE` retirés ; la
figure des lieux montre un sous-dossier entier puis un chemin replié).

Un effet de bord : la capsule a perdu une vingtaine de colonnes et tenait
entière à 90, si bien que les figures de largeur ne montraient plus de repli.
Elles passent de 120 / 90 / 60 à **120 / 70 / 40** — un rang, coupe entre
compartiments, coupe entre segments —, `largeur-90.svg` et `largeur-60.svg`
laissant place à `largeur-70.svg` et `largeur-40.svg`. Figures régénérées,
copie préalable du dossier dans `.backups\20260923-160332_avant-docs-depot\`.

## 26. Les mesures une à une — 24/09/2026

La question était ouverte — « peut-on encore améliorer le rendu ? » — et la
réponse a pris la forme d'un canevas de sept planches, chaque ligne dessinée
cellule par cellule aux teintes exactes du crate : la capsule telle qu'elle
était, cinq pistes, une recommandation. **A**, un palier par mesure ; **B**, un
cadre qui porte l'alerte ; **C**, une jauge fine ; **D**, l'effort en escalier ;
**E**, moins de rangs. L'utilisateur a retenu A + B + C ; D et E restent notées.
Le lot a été rédigé avant toute ligne de code, `.claude\CHANTIER-statusline-mesures.md`,
et ses questions tranchées une à une sur la capture d'un script de comparaison
collé dans Windows Terminal.

### Ce que la capsule taisait

Le compartiment des mesures prenait le fond du **pire** palier qu'il portait.
Une fenêtre de 5 heures épuisée mettait donc au rouge un contexte à 34 % et une
semaine à 45 % : le fond disait qu'il fallait lever le pied, pas de quoi. Depuis
la 2.4.0, chaque mesure prend le fond de son propre palier (A), et son cadre —
les deux bords qui la coiffent, l'arc qui la ferme — l'encre de ses valeurs
(B) : l'alerte se voit du coin de l'œil, et elle se voit **là où elle est**.

### Deux invariants, tenus par construction

Le compartiment des mesures reste **un** compartiment, de fond
`Fond::ParPalier` : en couleur, `encapsuler` le pose tranche par tranche, un
segment par tranche, jointes par un arc ; sous `NO_COLOR`, il est un
compartiment comme un autre, joint par le point. Deux conséquences, qu'aucun
code n'a eu à garantir :

- **la largeur ne change pas** : le point et ses deux blancs font trois
  cellules, le liseré, l'arc et le liseré aussi. `largeur_capsule` et
  `empaqueter` n'ont pas su qu'un compartiment s'éclatait, et le repli coupe
  aux mêmes endroits — `eclater_ne_change_pas_la_largeur` le vérifie à 120,
  40, 30 et 20 colonnes ;
- **la ligne `NO_COLOR` ne change pas d'un octet** : l'oracle n'a pas été
  dégelé, et le harnais n'a rien eu à apprendre.

L'autre voie — trois compartiments distincts — aurait demandé une règle de
plus sous `NO_COLOR` (les joindre par le point, et non par l'espace des
compartiments) et une notion de groupe dans l'assemblage.

### La jauge fine, et ce que l'écran a démenti

Le §13 tenait la jauge à seize crans pour hors d'atteinte sans changer de
police : les blocs partiels `▏▎▍▌▋▊▉` sont absents de Consolas, et le 25/08 les
blocs de hauteur, absents de même, sortaient d'une police de repli. Mais le
§23 a appris depuis que Windows Terminal trace lui-même les caps Powerline.
Le préalable du chantier a posé la question pour les huitièmes, et la capture,
relevée au pixel, a répondu : **1, 2, 3, 5, 6, 7, 8 et 9 pixels** sur une cellule
de 9, huit largeurs distinctes, tracées par le terminal.

La jauge des fenêtres compte donc seize crans sur ses deux mêmes cellules, un
tous les 6,25 points. Le vide n'est plus un `░` gris mais une **rainure** : le
fond des deux cellules, un ton au-dessus de celui de la mesure. Trois variantes
ont été montrées — rainure sombre, claire, aucune. La capture a appris que
le fond du terminal sous l'acrylique varie d'une ligne à l'autre (de
`37,37,36` à `56,53,49` sur la même image) et que la rainure sombre
(`41;45;52`) en prenait presque la teinte : elle se lisait comme une fente dans
la capsule. L'utilisateur a retenu la claire, contre la recommandation ; celle
de l'ardoise est l'ancienne piste `#525965`, passée de l'encre au fond. Au
passage, la question que le §23 laissait ouverte — `adjustIndistinguishableColors`
relèvera-t-il une piste à 1,5:1 de l'ardoise ? — perd son objet, d'après la
description du réglage, qui ne touche qu'aux avant-plans ; ce n'est pas
vérifié à l'écran.

L'arrondi va au plus proche, avec deux bornes : un huitième dès 1 %, le plein
à 100 % seulement — une jauge pleine dirait le plafond atteint. Sous
`NO_COLOR`, la jauge garde ses sept crans : sans fond, pas de rainure, et une
valeur de 10 % ne serait qu'un trait isolé.

### Deux exceptions à la règle du fond

La règle 1 de la capsule — le fond appartient au compartiment, les fragments
ne touchent qu'à l'avant-plan — a désormais deux exceptions, et deux
seulement : un compartiment `Fond::ParPalier` pose un fond par tranche ; la
jauge fine pose le fond de sa rainure, puis **rend** celui de sa tranche. Pour
le rendre, elle doit le connaître au moment de s'écrire : le palier d'une
fenêtre se calcule donc avant sa mise en forme, et `formater_fenetre` calcule
le rythme — qui peut porter une projection en alerte sur une valeur calme —
avant la mesure.

### Le cadre, tranche par tranche

Chaque tranche a son encre de contour : le gris sous les seuils, l'ambre ou le
corail au-delà — les encres mêmes des valeurs, un test y veille. L'arc entre
deux tranches prend l'encre du **pire** des deux voisins : géométriquement, il
ferme la tranche de gauche, mais une mesure critique bordée d'un arc gris
paraissait ouverte. Les bords s'écrivent tronçon par tronçon, une séquence
d'encre à chaque changement seulement, et toujours ouverts par l'encre avant le
blanc qui saute le cap : sans palier franchi, la sortie est octet pour octet
celle de la 2.3.0 — `les_bords_du_cadre_encadrent_la_pilule_entre_les_caps`
n'a pas bougé.

### Une recommandation retirée dans l'heure

Le chantier recommandait de donner sa jauge au contexte, et l'utilisateur
l'avait acceptée. La relecture de `reglages.rs` a trouvé, au commentaire de
`BLOCS_JAUGE`, la règle écrite le 26/08 : une fenêtre se remplit vers un
plafond qu'on subit, le contexte se compacte — lui donner une jauge suggérerait
une fatalité qu'il n'a pas. La question reposée avec cet argument,
l'utilisateur a gardé la règle. La largeur au calme reste à 85 cellules.

### La règle du §6, une dixième fois — et sans le dégel

Rien ne bouge sous `NO_COLOR`, et l'oracle reste gelé : **0 divergence sur 109
cas**, les 26 cas colorés sautés. Ceux-ci ont été relus contre un relevé de
l'exe 2.3.0 fait avant d'écrire (`.backups\harnais-reference-exe-2.3.0_20260924.txt`) :
séquences, blocs et heures neutralisés, **0 écart de texte** sur les 135 cas —
les heures, parce que le harnais les calcule sur l'horloge, et que vingt et une
minutes séparaient les deux passages. Les 26 pilules gardent leurs deux remises
à zéro. **93 tests** (86, huit neufs, un remplacé), verts avec et sans
`NO_COLOR` ; clippy `-D warnings` et `fmt` propres. Version **2.4.0**, copie
des sources dans `.backups\20260924-114806_avant-mesures\`.

### Le guide

`generer-guide.ps1` a appris les huitièmes — une barre collée à gauche, sur
le fond de la cellule — et une autre façon de placer les légendes : les plages
de fond contiguës ne délimitent plus les tranches, puisque la rainure change de
fond au milieu d'une mesure et que deux mesures calmes partagent la même
ardoise ; une tranche commence désormais à chaque jonction. L'anatomie porte six
légendes au lieu de quatre, la figure de la jauge montre 0, 3, 20, 45, 56, 78,
97 et 100 %, celle des paliers une quatrième ligne où deux mesures ont chacune
le leur. Légendes et texte relus, copie préalable dans
`.backups\20260924-121533_avant-docs-mesures\`.

### Ce qui reste ouvert

- **Les états d'alerte dans Claude Code.** Le binaire a été validé à l'écran
  au calme ; fonds ambre et rouge, cadre coloré et rainures de palier n'ont été
  vus que dans Windows Terminal et dans les tests. Le mécanisme est celui des
  jonctions, que Claude Code relaie : à confirmer au premier seuil franchi.
- **`adjustIndistinguishableColors` et les fonds**, voir plus haut : d'après
  la description, pas d'après l'écran.
- **Les pistes D et E**, notées au canevas : l'effort en cinq marches de largeur
  fixe, qui ôterait à l'emplacement sa dérive de trois colonnes, et une capsule
  à deux rangs ou à un seul, qui rendrait la hauteur que le cadre coûte.
