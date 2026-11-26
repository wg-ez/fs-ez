# shellcheck shell=bash
# Absence de service de contenu : pas de téléchargement, pas de média, pas de Range.

audit_no_content() {
  section "Absence de contenu"

  local path
  for path in "/health" "/login" "/" "/tags" "/views" "/views/new"; do
    if [ "$path" = "/health" ] || [ "$path" = "/login" ]; then
      request GET "$path"
    else
      session_request GET "$path"
    fi
    expect_header_absent "contenu: pas de Content-Disposition $path" "content-disposition"
    expect_header_absent "contenu: pas d'Accept-Ranges $path" "accept-ranges"
  done

  for path in "/api/files" "/api/tags" "/api/views" "/api/groups" "/api/status" "/api/tokens" "/api/session"; do
    session_request GET "$path"
    expect_status "contenu: $path" "$HTTP_STATUS" "200"
    expect_header_absent "contenu: pas de Content-Disposition $path" "content-disposition"
    expect_header_absent "contenu: pas d'Accept-Ranges $path" "accept-ranges"
  done

  for path in "/static/css/app.css" "/static/js/app.js" "/static/vendor/htmx.min.js"; do
    request GET "$path"
    expect_header_absent "contenu: pas de Content-Disposition $path" "content-disposition"
    expect_header_absent "contenu: pas d'Accept-Ranges $path" "accept-ranges"
  done

  audit_no_content_stream
  audit_no_content_user_file

  for path in "/media" "/download" "/preview" "/raw" "/file" "/download/1" "/preview/1"; do
    session_request GET "$path"
    expect_status "contenu: route $path absente" "$HTTP_STATUS" "404"
  done

}

audit_no_content_stream() {
  : >"$AUDIT_TMP/body"
  : >"$AUDIT_TMP/headers"
  HTTP_STATUS="$(curl -sS --path-as-is --max-time 2 -o "$AUDIT_TMP/body" -D "$AUDIT_TMP/headers" -w '%{http_code}' -b "$SESSION_COOKIE" "$BASE_URL/api/events" 2>/dev/null || true)"
  HTTP_BODY="$AUDIT_TMP/body"
  HTTP_HEADERS="$AUDIT_TMP/headers"
  if [ "$HTTP_STATUS" = "200" ]; then
    pass "contenu: /api/events est un flux SSE"
  else
    fail "contenu: /api/events est un flux SSE" "statut ${HTTP_STATUS:-000}"
  fi
  expect_header_contains "contenu: SSE content-type" "content-type" "text/event-stream"
  expect_header_absent "contenu: SSE sans Content-Disposition" "content-disposition"
  expect_header_absent "contenu: SSE sans Accept-Ranges" "accept-ranges"
}

audit_no_content_user_file() {
  if [ -z "$AUDIT_FILES" ] || [ ! -d "$AUDIT_FILES" ]; then
    skip "contenu: fichier utilisateur indexé" "AUDIT_FILES absent"
    return
  fi
  local name="audit-content-$(audit_unique).pdf"
  local source="$AUDIT_FILES/$name"
  printf '%%PDF-1.4\nAUDIT_USER_CONTENT_MARKER %s\n' "$(audit_unique)" >"$source"
  session_request POST "/api/inbox/scan"
  expect_status "contenu: scan du dossier files" "$HTTP_STATUS" "200"
  session_request GET "/api/files?q=$name"
  expect_status "contenu: fichier utilisateur listé" "$HTTP_STATUS" "200"
  local id
  id="$(json_number id)"
  if [ -z "$id" ]; then
    fail "contenu: fichier utilisateur identifié"
    rm -f "$source"
    return
  fi

  session_request GET "/api/files/$id"
  expect_status "contenu: métadonnées du fichier" "$HTTP_STATUS" "200"
  expect_header_absent "contenu: métadonnées sans Content-Disposition" "content-disposition"
  expect_header_absent "contenu: métadonnées sans Accept-Ranges" "accept-ranges"
  expect_body_not_contains "contenu: métadonnées sans contenu de fichier" "AUDIT_USER_CONTENT_MARKER"
  expect_header_contains "contenu: métadonnées JSON" "content-type" "application/json"

  session_request GET "/files/$id"
  expect_status "contenu: page détail du fichier" "$HTTP_STATUS" "200"
  expect_header_absent "contenu: détail sans Content-Disposition" "content-disposition"
  expect_header_absent "contenu: détail sans Accept-Ranges" "accept-ranges"
  expect_body_not_contains "contenu: détail sans contenu de fichier" "AUDIT_USER_CONTENT_MARKER"
  local content_type
  content_type="$(header_value "$HTTP_HEADERS" content-type)"
  if printf '%s' "$content_type" | grep -Eqi '^(image|video|audio)/|application/pdf'; then
    fail "contenu: détail jamais servi comme média" "$content_type"
  else
    pass "contenu: détail jamais servi comme média"
  fi

  session_request GET "/api/files/$id" -H "Range: bytes=0-3"
  expect_header_absent "contenu: métadonnées ignorent Range" "content-range"
  expect_body_not_contains "contenu: Range ne sert pas le fichier" "AUDIT_USER_CONTENT_MARKER"

  session_request GET "/api/files/$id/download"
  expect_status "contenu: pas de route de téléchargement par id" "$HTTP_STATUS" "404"
  rm -f "$source"
}
