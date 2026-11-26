#!/usr/bin/env bash
# Portes fmt / clippy / test en `type=cacheonly` : rien n'est exporte, donc
# aucune image ni conteneur residuel. Le cache BuildKit reste alimente.
# `TEST_FILTER=<sous-chaine>` restreint la suite, vide = tous les tests.
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT"

gate() {
    local target="$1"
    shift
    docker build -f docker/Dockerfile --output type=cacheonly --target "$target" "$@" .
}

gate fmt
gate clippy
if [[ -n "${TEST_FILTER:-}" ]]; then
    gate test --build-arg "TEST_FILTER=$TEST_FILTER"
else
    gate test
fi

# htmx : binaire vendorise identique au pin upstream, conteneur jetable.
docker run --rm -v "$ROOT/static/vendor:/o:ro" curlimages/curl:latest \
    sh -c 'set -eu
        curl -sSfL -o /tmp/htmx.min.js "https://unpkg.com/htmx.org@2.0.11/dist/htmx.min.js"
        echo "d6fdc75f204e6bdefa99b69bf1e6d4ac69b8a364f77929f45c13476b4000f717  /tmp/htmx.min.js" | sha256sum -c -
        echo "d6fdc75f204e6bdefa99b69bf1e6d4ac69b8a364f77929f45c13476b4000f717  /o/htmx.min.js" | sha256sum -c -'
