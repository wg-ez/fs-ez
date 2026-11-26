# AGENTS.md

## Workflow

- Implémenter **une fiche à la fois**, dans l'ordre numérique. Les `*_GATE` doivent être vertes avant la phase suivante.
- Lire uniquement la fiche assignée et le code qu'elle cite. Inspecter l'existant avant d'écrire, ne pas réécrire sans nécessité.
- Les chemins, noms de modules, signatures et schémas existants sont des contrats. Les rubriques « Hors périmètre » sont bloquantes.
- Un sous-agent = une fiche. Terminer par le rapport demandé (fichiers, commandes, résultats, écarts, blocages).
- Pas de `git init`, pas de commit, sauf demande explicite.

## Vérifications

```sh
scripts/gates.sh               # fmt + clippy + test + htmx, sans conteneur résiduel
scripts/build.sh               # image runtime seule, cache BuildKit borné à 8 Go
scripts/build.sh --up          # idem + redémarre fs-ez
scripts/build.sh --gates --up  # portes + image + redémarrage
```

- `TEST_FILTER=<sous-chaîne>` pour un seul test.
- Docker indisponible : `cargo fmt --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --locked`.
- Aucun `target/` hôte, cache dans `/var/lib/docker`, `Cargo.lock` maintenu, builds `--locked`.
- `scripts/build.sh` sans `--gates` = itérations seulement, jamais avant livraison.
- Nettoyage : `scripts/docker-clean.sh` (`--all` = cache complet).

## Tests hors fichiers de code

- Pas de `#[cfg(test)] mod tests { … }` inline : fichier frère `tests.rs` (`src/auth/totp.rs` → `src/auth/totp/tests.rs`, la source ne déclare que `#[cfg(test)] mod tests;`).
- Module racine `src/lib.rs` → `src/tests.rs`.
- Plusieurs thèmes → répertoire `tests/` (`mod.rs` façade + un module par thème).
- `*_test.rs` (`src/layout_test.rs`) = fichier de test, autorisé à contenir des tests.
- Dans `tests.rs`, `super` = module parent (`use super::*;` valide).
- Garde exécutable : chaque crate déclare `#[cfg(test)] mod layout_test;`, qui appelle `fs_ez_test_support::layout::assert_tests_outside_sources` ; un test inline fait échouer la suite.
- Le déplacement ne change ni les noms, ni les assertions, ni le nombre de tests.

## Architecture

- Workspace Cargo à la racine, crates dans `crates/`, binaire final `crates/fs-ez`.
- Dépendances strictement descendantes : `fs-ez-app` ne dépend **jamais** de `rusqlite`, `axum` ou `fuser`. Le web et la CLI sont deux adaptateurs de `fs-ez-app` ; aucun appel shell ou HTTP entre eux.
- Traits de persistance synchrones (`&self`), stockés en `Arc<dyn Store>`. Tout appel bloquant côté web passe par `tokio::task::spawn_blocking`.
- SQLite est l'unique source de vérité. WAL, `foreign_keys=ON`, `busy_timeout=5s`, migrations sous `BEGIN IMMEDIATE`. Le serveur détecte les écritures CLI via `PRAGMA data_version`.
- Chemins stockés en BLOB (octets Unix) + `path_display` TEXT lossy. Une sortie JSON ne doit jamais échouer sur un chemin non-UTF8.
- IDs typés (`FileId`, `TagId`, …) dans l'API publique, jamais d'`i64` nu.
- `panic = "abort"` : toute entrée utilisateur reste en `Result`. `unsafe` interdit (`#![forbid(unsafe_code)]`). Pas de `unwrap`/`expect` sur un chemin atteignable par l'utilisateur.
- fs-ez ne déplace, ne copie, ne renomme et ne supprime jamais un fichier utilisateur. Aucun endpoint ne sert le contenu d'un fichier (pas d'aperçu, de téléchargement, de `Range`).
- FUSE : liens symboliques sous `tags/`, `views/`, `files/`, cibles éditables en écriture, namespace virtuel non modifiable. Le VFS tourne dans un processus séparé pour qu'un crash FUSE ne tue pas le serveur.
- Configuration : flags CLI > variables d'environnement > `<root>/fs-ez.toml` > réglages en base > défauts. Variables clés : `HOST`, `PORT`, `MAX_UPLOAD`, `USERNAME`, `PASSWORD`, `PUID`, `PGID`, `TZ`, `RUST_LOG`, `FS_EZ_CONFIG`. Le root est le CWD (`/data` dans l'image), l'état vit dans `FS_EZ_CONFIG` (`/config` dans l'image, `<root>/config` sinon).

## Choix techniques imposés

- Pas de `clap`, pas d'ORM, pas de SPA. Parsing CLI maison ; SQL préparé uniquement, jamais de concaténation.
- Web : axum + askama + htmx vendorisé. Aucun JSON côté UI, aucun script/style inline, aucun `on*=`. CSP `script-src 'self'`.
- Auth : un seul utilisateur, Argon2id pour le mot de passe, SHA-256 pour sessions et jetons d'API. CSRF via `Sec-Fetch-Site`.
- Couleurs de tags = jetons de palette (`blue`, `amber`…), pas de hex.
- Toute erreur API est `{ "error": "…" }` avec un code HTTP stable.
- Assets : `build.rs` pré-gzippe `static/` ; `static/css/app.css` est généré par Tailwind dans le stage Docker `css` et reste commité pour les builds hôte, ne pas l'éditer à la main.
- Commentaires et documentation interne en français ; interface i18n EN/FR (`locales/en.json` canonique).

## Commentaires

- **Uniquement si le code ne suffit pas** : expliquer le pourquoi, la contrainte, l'invariant, le piège ou la conséquence. Code lisible ou commentaire redondant → **supprimer**.
- **Une ligne physique** par commentaire, **français**, style **télégraphique one-liner**. Faits séparés par `.` ou `,`, **jamais `;`** ; `+`, `→`, `=` autorisés.
- **Zéro perte** : noms, chemins, chiffres, contraintes, causes, conséquences.
- `//`, `///`, `//!`, `/* … */` : même règle. Exception : fence Markdown, doc-test, tableau ou liste structurée indispensable → structure conservée.
- Commentaires d'étape (`// 1.`, `// Step`, `// Étape`) **inchangés**.
