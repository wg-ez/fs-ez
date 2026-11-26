# shellcheck shell=bash
# Traversée de chemins : statiques, API et liens symboliques sortants.

audit_traversal() {
  section "Traversée et chemins"

  local path
  for path in "/static/../Cargo.toml" "/static/%2e%2e/Cargo.toml" "/static/..%2fCargo.toml"; do
    request GET "$path"
    expect_status "traversée: $path -> 404" "$HTTP_STATUS" "404"
    expect_body_not_contains "traversée: aucun Cargo.toml $path" "[workspace]"
  done

  session_request GET "/api/files/../../etc/passwd"
  expect_status "traversée: /api/files/../../etc/passwd" "$HTTP_STATUS" "400 404"
  expect_body_not_contains "traversée: aucun contenu passwd" "root:"

  session_request GET "/api/files/%2e%2e%2f%2e%2e%2fetc%2fpasswd"
  expect_status "traversée: identifiant API encodé" "$HTTP_STATUS" "400 404"
  expect_body_not_contains "traversée: aucun contenu passwd encodé" "root:"

  audit_traversal_symlink
  audit_traversal_names
}

audit_traversal_symlink() {
  if [ -z "$AUDIT_FILES" ] || [ ! -d "$AUDIT_FILES" ]; then
    skip "traversée: lien sortant ignoré" "AUDIT_ROOT absent"
    return
  fi
  local link="$AUDIT_FILES/audit-outside-link-$(audit_unique)"
  if ! ln -s /etc/passwd "$link" 2>/dev/null; then
    skip "traversée: lien sortant ignoré" "création du lien impossible"
    return
  fi
  session_request POST "/api/inbox/scan"
  expect_status "traversée: scan du dossier files" "$HTTP_STATUS" "200"
  request GET "/api/files?q=audit-outside-link-$(audit_unique)" -b "$SESSION_COOKIE"
  expect_status "traversée: recherche du lien" "$HTTP_STATUS" "200"
  expect_body_contains "traversée: lien sortant non importé" '"items":[]'
  rm -f "$link"
}

audit_traversal_names() {
  local name="../audit-evil-$(audit_unique)"
  session_request POST "/api/tags" \
    -H "Content-Type: application/json" \
    -d "{\"name\":\"$name\",\"color\":\"blue\"}"
  if [ "$HTTP_STATUS" = "400" ]; then
    pass "traversée: tag avec ../ refusé"
  elif [ "$HTTP_STATUS" = "201" ]; then
    local stored
    stored="$(json_string name)"
    case "$stored" in
      *..*|*/*) fail "traversée: tag avec ../ refusé ou normalisé" ;;
      *) pass "traversée: tag avec ../ normalisé" ;;
    esac
  else
    fail "traversée: tag avec ../" "statut $HTTP_STATUS"
  fi

  session_request POST "/api/views" \
    -H "Content-Type: application/json" \
    -d "{\"name\":\"$name\",\"filter\":{\"op\":\"tag\",\"tag\":\"auditprobe\",\"negated\":false}}"
  if [ "$HTTP_STATUS" = "400" ]; then
    pass "traversée: nom de vue avec ../ refusé"
    return
  fi
  if [ "$HTTP_STATUS" != "201" ]; then
    fail "traversée: nom de vue avec ../" "statut $HTTP_STATUS"
    return
  fi
  local id
  id="$(json_number id)"
  session_request GET "/views"
  expect_status "traversée: page vues consultable" "$HTTP_STATUS" "200"
  if grep -Fq 'href="/views/../' "$HTTP_BODY"; then
    fail "traversée: nom de vue jamais utilisé dans une URL"
  else
    pass "traversée: nom de vue jamais utilisé dans une URL"
  fi
  if [ -n "$id" ]; then
    session_request DELETE "/api/views/$id"
  fi
}
