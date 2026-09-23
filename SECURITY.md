# Sécurité

## Signaler une vulnérabilité

Par le formulaire privé du dépôt : onglet **Security** → **Report a
vulnerability** (signalement privé activé). Pas d'issue publique, pas de pull
request qui décrirait la faille avant qu'elle soit corrigée.

Le dépôt n'a qu'un mainteneur, sans astreinte : comptez quelques jours pour
un premier retour. Un correctif est publié comme une nouvelle Release,
signalée dans les notes.

## Versions prises en charge

La dernière Release, et elle seule. Le binaire ne se met pas à jour tout seul
: pour savoir si le vôtre est le dernier, comparer son empreinte à celle de la
dernière Release (voir plus bas).

## Ce que le binaire lit, écrit et n'envoie pas

Le binaire est lancé par Claude Code à chaque rafraîchissement de la ligne de
statut. Il lit :

- le payload JSON que Claude Code lui passe sur l'entrée standard (modèle,
  contexte, coût, répertoire, version) ;
- `~/.claude.json`, pour le type d'abonnement et le dernier relevé `/usage`
  (`CLAUDE_STATUSLINE_CONFIG` en désigne un autre) ;
- le dossier `.git` le plus proche du répertoire courant, pour la branche ;
- son cache `claude-code\statusline-cache.json` sous `%LOCALAPPDATA%` (repli
  `%TEMP%`), pour projeter le rythme de consommation.

Il écrit ce seul cache, plus un journal de diagnostic si `analyser-journal.ps1`
l'a activé. Il n'ouvre **aucune connexion réseau**, ne lance aucun processus
et n'écrit rien ailleurs.

## Vérifier ce que vous téléchargez

- Chaque Release joint `statusline.exe.sha256`, et porte l'empreinte dans ses
  notes : `(Get-FileHash statusline.exe).Hash` (PowerShell) ou
  `certutil -hashfile statusline.exe SHA256` doit la retrouver.
- Les Releases sont immuables : ni l'asset ni le tag ne changent après
  publication.
- Les commits de `main` et les tags `v*` sont signés (clé SSH du compte,
  « Verified » sur GitHub) ; `git verify-tag vX.Y.Z` après clonage, avec la
  clé publique du compte dans votre `allowed_signers`.
- La CI (`.github/workflows/ci.yml`) tourne avec un jeton en lecture seule,
  n'exécute que des actions épinglées par SHA, et le dépôt exige cet
  épinglage ; CodeQL analyse les workflows et le crate (Rust) à chaque push.

Le binaire n'est pas signé Authenticode : SmartScreen peut avertir au premier
lancement d'un téléchargement. Compiler soi-même (`cargo build --release`
dans `statusline-rs/`) reste la voie la plus sûre.
