# shellcheck shell=bash
# Injection SQL, XSS stocké et en-têtes Host/X-Forwarded.

audit_injection() {
  section "Injection"

  session_request GET "/api/files?query=%27%20OR%201%3D1%20--"
  if [ "$HTTP_STATUS" = "400" ]; then
    pass "injection: requête SQL rejetée sans bypass"
  elif [ "$HTTP_STATUS" = "200" ] && grep -Fq '"items":[]' "$HTTP_BODY"; then
    pass "injection: requête SQL sans bypass"
  else
    fail "injection: requête SQL sans bypass" "statut $HTTP_STATUS"
  fi
  if grep -Eqi 'sql syntax|sqlite|SELECT |syntax error' "$HTTP_BODY" 2>/dev/null; then
    fail "injection: aucune erreur SQL exposée"
  else
    pass "injection: aucune erreur SQL exposée"
  fi

  audit_injection_tag
  audit_injection_group
  audit_injection_view
  audit_injection_host
}

audit_injection_tag() {
  local payload="<script>alert(1)</script>"
  session_request POST "/api/tags" \
    -H "Content-Type: application/json" \
    -d "{\"name\":\"$payload\",\"color\":\"blue\"}"
  if [ "$HTTP_STATUS" = "400" ]; then
    pass "injection: tag <script> refusé"
    return
  fi
  if [ "$HTTP_STATUS" != "201" ]; then
    fail "injection: tag <script>" "statut $HTTP_STATUS"
    return
  fi
  session_request GET "/tags"
  if grep -Fqi '<script' "$HTTP_BODY"; then
    fail "injection: tag <script> échappé"
  else
    pass "injection: tag <script> échappé"
  fi
}

audit_injection_group() {
  local payload="<img src=x onerror=alert(1)>-$(audit_unique)"
  session_request POST "/api/groups" \
    -H "Content-Type: application/json" \
    -d "{\"name\":\"$payload\"}"
  if [ "$HTTP_STATUS" = "400" ]; then
    pass "injection: groupe <img> refusé"
    return
  fi
  if [ "$HTTP_STATUS" != "201" ]; then
    fail "injection: groupe <img>" "statut $HTTP_STATUS"
    return
  fi
  local id
  id="$(json_number id)"
  session_request GET "/"
  if grep -Eqi '<img[^>]*onerror' "$HTTP_BODY" 2>/dev/null; then
    fail "injection: nom de groupe <img> échappé"
  else
    pass "injection: nom de groupe <img> échappé"
  fi
  if [ -n "$id" ]; then
    session_request DELETE "/api/groups/$id"
  fi
}

audit_injection_view() {
  local payload="<img src=x onerror=alert(1)>-$(audit_unique)"
  session_request POST "/api/views" \
    -H "Content-Type: application/json" \
    -d "{\"name\":\"$payload\",\"filter\":{\"op\":\"tag\",\"tag\":\"auditprobe\",\"negated\":false}}"
  if [ "$HTTP_STATUS" = "400" ]; then
    pass "injection: vue <img> refusée"
    return
  fi
  if [ "$HTTP_STATUS" != "201" ]; then
    fail "injection: vue <img>" "statut $HTTP_STATUS"
    return
  fi
  local id
  id="$(json_number id)"
  session_request GET "/views"
  if grep -Eqi '<img[^>]*onerror' "$HTTP_BODY" 2>/dev/null; then
    fail "injection: nom de vue <img> échappé"
  else
    pass "injection: nom de vue <img> échappé"
  fi
  if [ -n "$id" ]; then
    session_request DELETE "/api/views/$id"
  fi
}

audit_injection_host() {
  request GET "/" -H "Host: evil.example" -H "X-Forwarded-Host: evil.example" -H "X-Forwarded-Proto: https"
  local location
  location="$(header_value "$HTTP_HEADERS" location)"
  case "$location" in
    *evil.example*) fail "injection: Host ignoré dans Location" ;;
    *) pass "injection: Host ignoré dans Location" ;;
  esac

  session_request GET "/" -H "Host: evil.example" -H "X-Forwarded-Host: evil.example" -H "X-Forwarded-Proto: https"
  expect_status "injection: page avec Host forgé" "$HTTP_STATUS" "200"
  if grep -Fq "evil.example" "$HTTP_BODY" 2>/dev/null; then
    fail "injection: Host/X-Forwarded jamais utilisés dans les URLs"
  else
    pass "injection: Host/X-Forwarded jamais utilisés dans les URLs"
  fi
}
