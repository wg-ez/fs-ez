#!/usr/bin/env bash
# Audit HTTP de sécurité fs-ez : surface publique, auth, en-têtes, chemins, contenu et limites.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=audit.d/lib.sh
source "$SCRIPT_DIR/audit.d/lib.sh"
# shellcheck source=audit.d/auth.sh
source "$SCRIPT_DIR/audit.d/auth.sh"
# shellcheck source=audit.d/headers.sh
source "$SCRIPT_DIR/audit.d/headers.sh"
# shellcheck source=audit.d/api.sh
source "$SCRIPT_DIR/audit.d/api.sh"
# shellcheck source=audit.d/traversal.sh
source "$SCRIPT_DIR/audit.d/traversal.sh"
# shellcheck source=audit.d/no_content.sh
source "$SCRIPT_DIR/audit.d/no_content.sh"
# shellcheck source=audit.d/limits.sh
source "$SCRIPT_DIR/audit.d/limits.sh"
# shellcheck source=audit.d/injection.sh
source "$SCRIPT_DIR/audit.d/injection.sh"
# shellcheck source=audit.d/permissions.sh
source "$SCRIPT_DIR/audit.d/permissions.sh"

BASE_URL="${BASE_URL%/}"

if ! command -v curl >/dev/null 2>&1; then
  die "curl est requis"
fi
if [ -z "$AUDIT_PASS" ]; then
  die "AUDIT_PASS est requis (mot de passe de l'utilisateur de test)"
fi

init_tmp
trap cleanup_tmp EXIT

printf 'audit fs-ez\nbase: %s\nutilisateur: %s\n' "$BASE_URL" "$AUDIT_USER"
if [ -n "$AUDIT_CONFIG" ]; then
  printf 'état: %s\n' "$AUDIT_CONFIG"
fi

if ! audit_auth; then
  printf '\nRésumé: %d ok, %d échec, %d ignoré\n' "$PASS_COUNT" "$FAIL_COUNT" "$SKIP_COUNT"
  printf 'audit interrompu: connexion impossible\n' >&2
  exit 1
fi

audit_headers
audit_api
audit_traversal
audit_no_content
audit_limits
audit_injection
audit_permissions

printf '\nRésumé: %d ok, %d échec, %d ignoré\n' "$PASS_COUNT" "$FAIL_COUNT" "$SKIP_COUNT"
if [ "$FAIL_COUNT" -gt 0 ]; then
  printf 'Checks en échec:\n' >&2
  printf '  - %s\n' "${FAILED_CHECKS[@]}" >&2
  exit 1
fi
if [ "$SKIP_COUNT" -gt 0 ]; then
  printf 'audit: %d check(s) ignoré(s), voir les lignes skip.\n' "$SKIP_COUNT"
fi
exit 0
