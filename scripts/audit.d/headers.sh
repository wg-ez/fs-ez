# shellcheck shell=bash
# En-têtes de sécurité, CSP et absence de HTML inline.

audit_headers() {
  section "En-têtes et CSP"

  local path
  for path in "/health" "/login" "/static/css/app.css"; do
    request GET "$path"
    expect_status "headers: $path accessible" "$HTTP_STATUS" "200"
    check_security_headers "$path"
  done

  for path in "/" "/tags" "/views" "/views/new"; do
    session_request GET "$path"
    expect_status "headers: $path authentifié" "$HTTP_STATUS" "200"
    check_security_headers "$path"
    check_inline_html "$path"
  done

  request GET "/login"
  check_inline_html "/login"

  request GET "/static/css/app.css"
  expect_header_contains "headers: CSS content-type" "content-type" "text/css"
  request GET "/static/js/app.js"
  expect_header_contains "headers: JS content-type" "content-type" "text/javascript"
  request GET "/static/vendor/htmx.min.js"
  expect_header_contains "headers: htmx content-type" "content-type" "text/javascript"
}
