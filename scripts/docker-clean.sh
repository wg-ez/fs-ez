#!/usr/bin/env sh
# Purge les artefacts Docker du projet et borne le cache BuildKit.
# Bornes : `MAX_CACHE` (defaut 8 Go) et `MIN_FREE` (defaut 10 Go).
# `--all` purge tout le cache et retire le builder multiarch de release.
set -eu

ROOT="$(cd -- "$(dirname -- "$0")/.." && pwd)"
MAX_CACHE="${MAX_CACHE:-8GB}"
MIN_FREE="${MIN_FREE:-10GB}"

# Conteneurs et reseau du stack de dev, profils inclus.
docker compose -f "$ROOT/docker/compose.dev.yml" down --remove-orphans >/dev/null 2>&1 || true

# Images du projet, dont les portes taguees par d'anciens builds.
docker image rm -f fs-ez-dev fs-ez-fmt fs-ez-clippy fs-ez-test fs-ez-css \
    ghcr.io/fs-ez/fs-ez:latest >/dev/null 2>&1 || true

# Couches intermediaires orphelines (`<none>`) laissees par les rebuilds.
docker image prune -f >/dev/null

if [ "${1:-}" = "--all" ]; then
    # Cache BuildKit complet et builder multiarch separe (~5 Go).
    docker buildx prune -a -f --builder default >/dev/null
    if docker buildx inspect fs-ez-multiarch >/dev/null 2>&1; then
        docker buildx rm fs-ez-multiarch >/dev/null
    fi
else
    # Cache BuildKit du builder de dev : garde les builds recents, purge le reste.
    docker buildx prune -f --builder default \
        --max-used-space "$MAX_CACHE" --min-free-space "$MIN_FREE" >/dev/null
fi

docker system df
