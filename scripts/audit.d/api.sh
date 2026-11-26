# shellcheck shell=bash
# Contrat d'erreurs de l'API JSON : 404, 405, 400 bornés et aucune fuite interne.

audit_api() {
  section "API"

  session_request GET "/api/inconnu"
  expect_status "api: /api/inconnu -> 404" "$HTTP_STATUS" "404"
  expect_header_contains "api: 404 JSON" "content-type" "application/json"
  expect_body_contains "api: 404 code not_found" "\"code\":\"not_found\""

  session_request PUT "/api/files"
  expect_status "api: méthode non autorisée -> 405" "$HTTP_STATUS" "405"

  session_request POST "/api/tags" -H "Content-Type: application/json" -d '{invalide'
  expect_status "api: JSON invalide -> 400" "$HTTP_STATUS" "400"
  expect_body_contains "api: 400 code bad_request" "\"code\":\"bad_request\""
  audit_no_leak "api: JSON invalide sans fuite"

  session_request DELETE "/api/tags/audit-inexistant-$(audit_unique)"
  expect_status "api: tag inconnu -> 404" "$HTTP_STATUS" "404"
  audit_no_leak "api: tag inconnu sans fuite"

  session_request GET "/api/files/abc"
  expect_status "api: identifiant invalide -> 400" "$HTTP_STATUS" "400"
  audit_no_leak "api: identifiant invalide sans fuite"

  session_request GET "/api/files?limit=501"
  expect_status "api: limite hors bornes -> 400" "$HTTP_STATUS" "400"
  expect_body_contains "api: limite 400 JSON" "\"code\":\"bad_request\""

  session_request GET "/api/files?query=%27%20OR%201%3D1%20--"
  expect_status "api: requête invalide -> 400" "$HTTP_STATUS" "400"
  audit_no_leak "api: requête invalide sans fuite"

  session_request GET "/api/status"
  expect_status "api: statut JSON" "$HTTP_STATUS" "200"
  expect_header_contains "api: statut content-type" "content-type" "application/json"
  expect_body_contains "api: statut versionné" "\"schema\":1"
}
