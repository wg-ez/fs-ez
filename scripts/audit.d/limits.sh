# shellcheck shell=bash
# Bornes de corps, refus de méthode, 404 JSON et fuite d'informations.

audit_limits() {
  section "Limites et erreurs"

  local big="$AUDIT_TMP/big.json"
  {
    printf '{"name":"'
    head -c 2200000 /dev/zero | tr '\0' 'a'
    printf '","color":"blue"}'
  } >"$big"
  session_request POST "/api/tags" -H "Content-Type: application/json" --data-binary "@$big"
  expect_status "limites: JSON > 2 Mio -> 413" "$HTTP_STATUS" "413"
  expect_body_contains "limites: 413 payload_too_large" "\"code\":\"payload_too_large\""
}
