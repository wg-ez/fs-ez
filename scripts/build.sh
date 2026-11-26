#!/usr/bin/env bash
# Build rapide : image runtime seule, sans portes par defaut.
# `--gates` ajoute fmt/clippy/test avant le build.
# `--up` redemarre fs-ez sans dependances ni rebuild (`depends_on` ignore).
# Apres le build : conteneurs sortis supprimes et cache BuildKit borne.
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE=(docker compose -f docker/compose.dev.yml)
GATES=0
UP=0

for arg in "$@"; do
    case "$arg" in
        --gates) GATES=1 ;;
        --up) UP=1 ;;
        *) echo "option inconnue: $arg" >&2; exit 2 ;;
    esac
done

cd "$ROOT"
if [[ "$GATES" == 1 ]]; then
    "$ROOT/scripts/gates.sh"
fi
"${COMPOSE[@]}" build fs-ez

# Aucun conteneur de porte ne doit survivre a un build.
docker ps -aq --filter "label=com.docker.compose.project=fs-ez-dev" \
    --filter "status=exited" | xargs -r docker rm >/dev/null

# Borne le cache BuildKit sans perdre les couches du build courant.
if docker buildx inspect default >/dev/null 2>&1; then
    docker buildx prune -f --builder default \
        --max-used-space "${MAX_CACHE:-8GB}" >/dev/null
fi

if [[ "$UP" == 1 ]]; then
    "${COMPOSE[@]}" up --no-deps --no-build -d fs-ez
fi
