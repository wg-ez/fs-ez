# shellcheck shell=bash
# Checks d'authentification, de session, de limitation et de jetons d'API.

audit_auth() {
  section "Auth"

  request GET "/"
  expect_status "auth: / sans cookie -> 303" "$HTTP_STATUS" "303"
  expect_header_matches "auth: redirection vers /login" "location" '^/login(\?|$)'

  request GET "/api/files"
  expect_status "auth: /api/files sans auth -> 401" "$HTTP_STATUS" "401"
  expect_header_contains "auth: 401 JSON" "content-type" "application/json"
  expect_body_contains "auth: 401 code unauthorized" "\"code\":\"unauthorized\""

  request POST "/login" \
    -H "Content-Type: application/x-www-form-urlencoded" \
    --data-urlencode "username=$AUDIT_USER" \
    --data-urlencode "password=fs-ez-audit-wrong-password"
  expect_status "auth: mauvais mot de passe -> 200" "$HTTP_STATUS" "200"
  if grep -Fq "Invalid credentials" "$HTTP_BODY" || grep -Fq "Identifiants invalides" "$HTTP_BODY"; then
    pass "auth: message de login générique"
  else
    fail "auth: message de login générique"
  fi
  if grep -Eqi 'utilisateur inconnu|unknown user|does not exist|no such' "$HTTP_BODY"; then
    fail "auth: aucun détail sur l'existence du compte"
  else
    pass "auth: aucun détail sur l'existence du compte"
  fi

  if ! login_session; then
    fail "auth: login correct" "statut $HTTP_STATUS"
    return 1
  fi
  pass "auth: login correct"

  expect_header_contains "auth: cookie HttpOnly" "set-cookie" "HttpOnly"
  expect_header_contains "auth: cookie SameSite=Lax" "set-cookie" "SameSite=Lax"
  expect_header_contains "auth: cookie Path=/" "set-cookie" "Path=/"
  if printf '%s' "$LAST_SET_COOKIE" | grep -qi 'Domain='; then
    fail "auth: cookie sans Domain"
  else
    pass "auth: cookie sans Domain"
  fi
  case "$BASE_URL" in
    https://*)
      expect_header_contains "auth: cookie Secure en HTTPS" "set-cookie" "Secure"
      ;;
    *)
      if printf '%s' "$LAST_SET_COOKIE" | grep -qi 'Secure'; then
        fail "auth: cookie non Secure en HTTP direct"
      else
        pass "auth: cookie non Secure en HTTP direct"
      fi
      ;;
  esac

  local primary="$SESSION_COOKIE"
  if login_session; then
    local secondary="$SESSION_COOKIE"
    request POST "/logout" -b "$secondary" -H "Sec-Fetch-Site: same-origin"
    expect_status "auth: logout -> 303" "$HTTP_STATUS" "303"
    expect_header_contains "auth: logout efface le cookie" "set-cookie" "Max-Age=0"
    request GET "/api/session" -b "$secondary"
    expect_status "auth: session invalide après logout" "$HTTP_STATUS" "401"
  else
    fail "auth: seconde session pour tester le logout"
  fi
  SESSION_COOKIE="$primary"

  request GET "/api/session" -b "$SESSION_COOKIE"
  expect_status "auth: session principale toujours valide" "$HTTP_STATUS" "200"

  local already_limited=0
  local attempt
  for attempt in 1 2 3 4 5 6; do
    request POST "/login" \
      -H "Content-Type: application/x-www-form-urlencoded" \
      --data-urlencode "username=$AUDIT_USER" \
      --data-urlencode "password=fs-ez-audit-wrong-password"
    if [ "$attempt" = "1" ] && [ "$HTTP_STATUS" = "429" ]; then
      already_limited=1
      break
    fi
    if [ "$attempt" -le 5 ]; then
      expect_status "auth: tentative $attempt -> 200" "$HTTP_STATUS" "200"
    else
      expect_status "auth: 6e tentative -> 429" "$HTTP_STATUS" "429"
    fi
  done
  if [ "$already_limited" = "1" ]; then
    pass "auth: 6e tentative -> 429 (limiteur déjà actif)"
  fi
  if [ "$HTTP_STATUS" = "429" ]; then
    expect_header_matches "auth: 429 Retry-After" "retry-after" '^[1-9][0-9]*$'
  else
    fail "auth: 429 Retry-After" "dernier statut $HTTP_STATUS"
  fi

  if [ -n "$AUDIT_TOKEN" ]; then
    API_TOKEN="$AUDIT_TOKEN"
    pass "auth: jeton fourni par AUDIT_TOKEN"
  else
    local token_name="audit-session-$(audit_unique)"
    session_request POST "/api/tokens" \
      -H "Content-Type: application/json" \
      -d "{\"name\":\"$token_name\"}"
    if [ "$HTTP_STATUS" = "201" ]; then
      pass "auth: création d'un jeton par session"
      API_TOKEN="$(json_string token)"
    else
      fail "auth: création d'un jeton par session" "statut $HTTP_STATUS"
    fi
  fi
  if [ -z "$API_TOKEN" ]; then
    fail "auth: jeton Bearer disponible"
    return 0
  fi

  bearer_request GET "/api/files"
  expect_status "auth: Bearer valide -> 200" "$HTTP_STATUS" "200"

  request GET "/api/files" -H "Authorization: Bearer fs-ez-audit-invalid"
  expect_status "auth: Bearer invalide -> 401" "$HTTP_STATUS" "401"
  expect_body_contains "auth: Bearer invalide JSON" "\"code\":\"unauthorized\""

  audit_auth_expired_token
  audit_auth_revoked_token

  bearer_request GET "/api/tokens"
  expect_status "auth: Bearer ne peut pas lister les jetons" "$HTTP_STATUS" "403"
  bearer_request POST "/api/tokens" -H "Content-Type: application/json" -d '{"name":"interdit"}'
  expect_status "auth: Bearer ne peut pas créer de jeton" "$HTTP_STATUS" "403"
  bearer_request DELETE "/api/tokens/999999"
  expect_status "auth: Bearer ne peut pas révoquer de jeton" "$HTTP_STATUS" "403"

  return 0
}

audit_auth_expired_token() {
  local expires
  expires="$(date -u -d '+3 seconds' '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || true)"
  if [ -z "$expires" ]; then
    skip "auth: Bearer expiré -> 401" "date GNU indisponible"
    return
  fi
  session_request POST "/api/tokens" \
    -H "Content-Type: application/json" \
    -d "{\"name\":\"audit-expire-$(audit_unique)\",\"expires_at\":\"$expires\"}"
  if [ "$HTTP_STATUS" != "201" ]; then
    fail "auth: création d'un jeton expirant" "statut $HTTP_STATUS"
    return
  fi
  local token id
  token="$(json_string token)"
  id="$(json_number id)"
  sleep 4
  request GET "/api/files" -H "Authorization: Bearer $token"
  expect_status "auth: Bearer expiré -> 401" "$HTTP_STATUS" "401"
  if [ -n "$id" ]; then
    session_request DELETE "/api/tokens/$id"
  fi
}

audit_auth_revoked_token() {
  session_request POST "/api/tokens" \
    -H "Content-Type: application/json" \
    -d "{\"name\":\"audit-revoque-$(audit_unique)\"}"
  if [ "$HTTP_STATUS" != "201" ]; then
    fail "auth: création d'un jeton à révoquer" "statut $HTTP_STATUS"
    return
  fi
  local token id
  token="$(json_string token)"
  id="$(json_number id)"
  if [ -z "$id" ]; then
    fail "auth: jeton à révoquer identifié"
    return
  fi
  session_request DELETE "/api/tokens/$id"
  expect_status "auth: révocation par session" "$HTTP_STATUS" "200"
  request GET "/api/files" -H "Authorization: Bearer $token"
  expect_status "auth: Bearer révoqué -> 401" "$HTTP_STATUS" "401"
}
