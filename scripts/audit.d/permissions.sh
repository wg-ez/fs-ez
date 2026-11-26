# shellcheck shell=bash
# Permissions du répertoire d'état et de la base ; logs sans secret.

audit_permissions() {
  section "Fichiers et permissions"

  if [ -z "$AUDIT_CONFIG" ]; then
    skip "permissions: dossier d'état en 0700" "AUDIT_CONFIG absent"
    skip "permissions: base en 0600" "AUDIT_CONFIG absent"
    audit_permissions_logs
    return
  fi

  local state_dir="$AUDIT_CONFIG"
  local mode
  mode="$(file_mode "$state_dir")"
  if [ "$mode" = "700" ]; then
    pass "permissions: dossier d'état en 0700"
  else
    fail "permissions: dossier d'état en 0700" "mode ${mode:-absent}"
  fi

  local db="${AUDIT_DB:-$state_dir/fs-ez.db}"
  if [ -e "$db" ]; then
    mode="$(file_mode "$db")"
    if [ "$mode" = "600" ]; then
      pass "permissions: base en 0600"
    else
      fail "permissions: base en 0600" "mode ${mode:-inconnu}"
    fi
  else
    fail "permissions: base présente" "$db"
  fi

  audit_permissions_logs
}

audit_permissions_logs() {
  if [ -z "$AUDIT_LOG" ] || [ ! -r "$AUDIT_LOG" ]; then
    skip "permissions: logs sans secret" "AUDIT_LOG absent"
    return
  fi
  if [ -n "$AUDIT_PASS" ] && grep -Fq -- "$AUDIT_PASS" "$AUDIT_LOG" 2>/dev/null; then
    fail "permissions: aucun mot de passe dans les logs"
  else
    pass "permissions: aucun mot de passe dans les logs"
  fi
  if grep -Eq 'fs-ez_session=[A-Za-z0-9_-]{16,}|Bearer [A-Za-z0-9_-]{16,}' "$AUDIT_LOG" 2>/dev/null; then
    fail "permissions: aucun jeton dans les logs"
  else
    pass "permissions: aucun jeton dans les logs"
  fi
  if grep -Eq '\$argon2(id|i|d)\$' "$AUDIT_LOG" 2>/dev/null; then
    fail "permissions: aucun hash de mot de passe dans les logs"
  else
    pass "permissions: aucun hash de mot de passe dans les logs"
  fi
  if grep -Fq 'AUDIT_USER_CONTENT_MARKER' "$AUDIT_LOG" 2>/dev/null; then
    fail "permissions: aucun contenu de fichier dans les logs"
  else
    pass "permissions: aucun contenu de fichier dans les logs"
  fi
}
