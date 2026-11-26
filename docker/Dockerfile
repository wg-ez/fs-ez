# syntax=docker/dockerfile:1
# fs-ez — Dockerfile unique. Seul moyen de compiler : aucun bind-mount de
# l'arbre source, cache cargo uniquement en cache mounts BuildKit, donc aucun
# `target/` ni `node_modules/` cote hote. `static/css/app.css` reste commite
# pour les builds hote, le stage `css` le regenere a l'identique.
# Stages : css, rustbase, devbase, fmt, clippy, test, builder musl, runtime, audit.

# ---------------------------------------------------------------- css ----
# Node uniquement ici : `npm ci` sur le lockfile commite, puis Tailwind CLI.
FROM node:alpine AS css
WORKDIR /work
COPY docker/package.json docker/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci --no-audit --no-fund
COPY static/css/ ./static/css/
RUN ./node_modules/.bin/tailwindcss -i ./static/css/input.css -o ./static/css/app.css --minify

# ------------------------------------------------------------ rustbase ----
# `rust-toolchain.toml` demande le canal `stable`, absent de l'image `rust:1.99.1`.
# Sans installation prealable, rustup retelecharge un toolchain (~15 s, ~150 Mo) par stage.
# Tag fige : `rust:latest` peut changer de version et invalider tous les layers.
FROM rust:1.99.1 AS rustbase
RUN rustup toolchain install stable --profile minimal \
      --component rustfmt --component clippy --component llvm-tools-preview \
      --target x86_64-unknown-linux-musl --target aarch64-unknown-linux-musl && \
    rustup default stable && \
    rustup toolchain uninstall 1.99.1

# ------------------------------------------------------------- devbase ----
# Socle glibc des portes et du serveur de dev : toolchain + sources + `app.css`
# frais du stage `css`. Cible cargo hors arbre source (`/cargo-target`).
FROM rustbase AS devbase
WORKDIR /work
ENV CARGO_TARGET_DIR=/cargo-target
# Incremental desactive : jamais reutilise entre deux suites completes et il pese
# plusieurs Go dans le cache mount `/cargo-target`, cf. `.cargo/config.toml` hote.
ENV CARGO_INCREMENTAL=0
# Manifestes seuls d'abord : une modif de source n'invalide pas le registre.
# Un `COPY` de wildcard aplatit les fichiers (meme basename Cargo.toml), d'ou la liste explicite.
COPY rust-toolchain.toml askama.toml Cargo.toml Cargo.lock build.rs ./
COPY .cargo/ ./.cargo/
COPY crates/fs-ez/Cargo.toml ./crates/fs-ez/Cargo.toml
COPY crates/fs-ez-app/Cargo.toml ./crates/fs-ez-app/Cargo.toml
COPY crates/fs-ez-cli/Cargo.toml ./crates/fs-ez-cli/Cargo.toml
COPY crates/fs-ez-core/Cargo.toml ./crates/fs-ez-core/Cargo.toml
COPY crates/fs-ez-fingerprint/Cargo.toml ./crates/fs-ez-fingerprint/Cargo.toml
COPY crates/fs-ez-fs/Cargo.toml ./crates/fs-ez-fs/Cargo.toml
COPY crates/fs-ez-query/Cargo.toml ./crates/fs-ez-query/Cargo.toml
COPY crates/fs-ez-sqlite/Cargo.toml ./crates/fs-ez-sqlite/Cargo.toml
COPY crates/fs-ez-store/Cargo.toml ./crates/fs-ez-store/Cargo.toml
COPY crates/fs-ez-test-support/Cargo.toml ./crates/fs-ez-test-support/Cargo.toml
COPY crates/fs-ez-web/Cargo.toml ./crates/fs-ez-web/Cargo.toml
# `cargo fetch` exige une cible par crate : stubs vides jetes juste apres le fetch.
# Aucune compilation avec ces stubs, les vraies sources arrivent au COPY suivant.
RUN for m in crates/*/Cargo.toml; do d=${m%/Cargo.toml}; mkdir -p "$d/src"; : > "$d/src/lib.rs"; : > "$d/src/main.rs"; done
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo fetch --locked && rm -rf crates/*/src
COPY crates/ ./crates/
COPY templates/ ./templates/
COPY locales/ ./locales/
COPY static/ ./static/
COPY --from=css /work/static/css/app.css ./static/css/app.css
# Pas de `touch` global : il daterait toutes les sources et forcerait la recompilation des 11 crates.
# `COPY` preserve la mtime hote : seule la crate dont une source change est recompilee.

# Porte fmt : ne compile rien, n'ecrit rien.
FROM devbase AS fmt
RUN cargo fmt --check

# Porte clippy : warnings = erreurs. Une source modifiee invalide les COPY
# en amont, la couche est donc rejouee sans CACHEBUST (rejeu inutile sinon).
FROM devbase AS clippy
RUN --mount=type=cache,target=/cargo-target,sharing=locked \
    cargo clippy --all-targets --all-features --locked -- -D warnings

# Porte test : `TEST_FILTER` restreint a un sous-ensemble, vide = tous.
FROM devbase AS test
ARG TEST_FILTER=""
# Les tests fs-ez-cli lancent le binaire fs-ez : le construire avant la suite evite une course.
RUN --mount=type=cache,target=/cargo-target,sharing=locked \
    cargo build --locked --bin fs-ez && \
    cargo test --locked $TEST_FILTER

# ------------------------------------------------------------- builder ----
# Release musl statique : `rusqlite` bundled compile l'amalgame SQLite avec
# `musl-gcc`. TARGETARCH = amd64|arm64.
FROM rustbase AS builder
ARG TARGETARCH
WORKDIR /work
ENV CARGO_TARGET_DIR=/work/target
# Pas de `rm -rf /var/lib/apt/lists/*` : le cache mount garde les listes, le
# nettoyage les supprimerait et forcerait un `apt-get update` reseau au rejeu.
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && \
    apt-get install -y --no-install-recommends gcc musl-tools musl-dev make perl
# Triple ecrit hors workspace : les COPY suivants ne l'invalident pas.
# Les deux cibles musl sont deja installees par `rustbase`.
RUN case "$TARGETARCH" in \
      amd64) target=x86_64-unknown-linux-musl ;; \
      arm64) target=aarch64-unknown-linux-musl ;; \
      *) echo "TARGETARCH non supporte: $TARGETARCH" >&2; exit 1 ;; \
    esac && \
    echo "$target" > /usr/local/share/fs-ez-target
COPY rust-toolchain.toml askama.toml Cargo.toml Cargo.lock build.rs ./
COPY .cargo/ ./.cargo/
COPY crates/fs-ez/Cargo.toml ./crates/fs-ez/Cargo.toml
COPY crates/fs-ez-app/Cargo.toml ./crates/fs-ez-app/Cargo.toml
COPY crates/fs-ez-cli/Cargo.toml ./crates/fs-ez-cli/Cargo.toml
COPY crates/fs-ez-core/Cargo.toml ./crates/fs-ez-core/Cargo.toml
COPY crates/fs-ez-fingerprint/Cargo.toml ./crates/fs-ez-fingerprint/Cargo.toml
COPY crates/fs-ez-fs/Cargo.toml ./crates/fs-ez-fs/Cargo.toml
COPY crates/fs-ez-query/Cargo.toml ./crates/fs-ez-query/Cargo.toml
COPY crates/fs-ez-sqlite/Cargo.toml ./crates/fs-ez-sqlite/Cargo.toml
COPY crates/fs-ez-store/Cargo.toml ./crates/fs-ez-store/Cargo.toml
COPY crates/fs-ez-test-support/Cargo.toml ./crates/fs-ez-test-support/Cargo.toml
COPY crates/fs-ez-web/Cargo.toml ./crates/fs-ez-web/Cargo.toml
# `cargo fetch` exige une cible par crate : stubs vides jetes juste apres le fetch.
RUN for m in crates/*/Cargo.toml; do d=${m%/Cargo.toml}; mkdir -p "$d/src"; : > "$d/src/lib.rs"; : > "$d/src/main.rs"; done
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo fetch --locked && rm -rf crates/*/src
COPY crates/ ./crates/
COPY templates/ ./templates/
COPY locales/ ./locales/
COPY static/ ./static/
COPY --from=css /work/static/css/app.css ./static/css/app.css
# Pas de touch global : il forcerait la recompilation des 11 crates, cf. devbase.
# Build depuis les vraies sources : aucune precompilation avec des stubs.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/work/target,sharing=locked \
    target="$(cat /usr/local/share/fs-ez-target)" && \
    cargo build --release --locked --target "$target" && \
    mkdir -p /out && \
    cp "/work/target/$target/release/fs-ez" /out/fs-ez
# Smoke tests : commande inconnue = code non nul + `unknown command`, version = succes.
RUN set -eu; \
    if /out/fs-ez commande-inconnue 2>/tmp/unknown.err; then echo "commande inconnue acceptee" >&2; exit 1; fi; \
    grep -q "unknown command" /tmp/unknown.err; \
    /out/fs-ez version >/dev/null

# ------------------------------------------------------------- runtime ----
# Alpine minimal : pas de toolchain ni Node. Entrée root puis bascule PUID:PGID.
FROM alpine:latest AS runtime
# `tzdata` : `TZ=Europe/Paris` doit résoudre les règles DST pour l'heure des sauvegardes et les dates affichées.
RUN apk add --no-cache ca-certificates tzdata
COPY --from=builder /out/fs-ez /usr/local/bin/fs-ez
# Le handler remonte du CWD vers `/static` : assets + `app.css` frais du stage css.
# Le PUID final peut être n'importe quel uid : lecture pour tous.
COPY --from=builder /work/static/ /static/
RUN chmod -R a+rX /static
# Root = CWD `/data` : fichiers dans `/data/files`, état monté sur `/config`.
WORKDIR /data
# Le serveur démarre en root pour chowner /data + /config puis setuid vers PUID:PGID.
ENV HOST=0.0.0.0 PORT=8678 FS_EZ_CONFIG=/config TZ=Etc/UTC
ENV PUID=1000 PGID=1000
EXPOSE 8678/tcp
VOLUME ["/data", "/config"]
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD wget -qO- http://127.0.0.1:8678/health || exit 1
STOPSIGNAL SIGTERM
ENTRYPOINT ["fs-ez"]
CMD ["serve"]

# --------------------------------------------------------------- audit ----
# Checks HTTP de la fiche 43, service `audit` du compose dev.
FROM alpine:latest AS audit
RUN apk add --no-cache bash curl python3
COPY scripts/ /scripts/
ENTRYPOINT ["bash", "/scripts/audit.sh"]
