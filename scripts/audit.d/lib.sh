# shellcheck shell=bash
# Bibliothèque commune de l'audit HTTP fs-ez : requêtes, compteurs, assertions.
# Aucune valeur secrète n'est affichée, seuls les noms de checks et les statuts le sont.

: "${BASE_URL:=http://127.0.0.1:8678}"
: "${AUDIT_USER:=user}"
: "${AUDIT_PASS:=}"
: "${AUDIT_TOKEN:=}"
: "${AUDIT_ROOT:=}"
: "${AUDIT_CONFIG:=${AUDIT_ROOT:+$AUDIT_ROOT/config}}"
: "${AUDIT_FILES:=}"
: "${AUDIT_DB:=}"
: "${AUDIT_LOG:=}"
: "${AUDIT_FAIL_FAST:=0}"

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
FAILED_CHECKS=()
SESSION_COOKIE=""
API_TOKEN=""
LAST_SET_COOKIE=""
HTTP_STATUS=""
HTTP_BODY=""
HTTP_HEADERS=""
AUDIT_TMP=""

if [ -n "$AUDIT_ROOT" ] && [ -z "$AUDIT_FILES" ]; then
  AUDIT_FILES="$AUDIT_ROOT/files"
fi

# shellcheck disable=SC2206
CURL=(curl -sS --path-as-is --max-time 20)

init_tmp() {
  AUDIT_TMP="$(mktemp -d "${TMPDIR:-/tmp}/fs-ez-audit.XXXXXX")"
  chmod 700 "$AUDIT_TMP"
}

# `$$` vaut 1 dans un conteneur : les noms fixes collisionnent d'un audit à l'autre.
audit_unique() {
  printf '%s%s' "$(date +%s%N)" "$RANDOM"
}

cleanup_tmp() {
  if [ -n "$AUDIT_TMP" ] && [ -d "$AUDIT_TMP" ]; then
    rm -rf "$AUDIT_TMP"
  fi
}

section() {
  printf '\n== %s ==\n' "$1"
}

pass() {
  PASS_COUNT=$((PASS_COUNT + 1))
  printf 'ok   %s\n' "$1"
}

fail() {
  FAIL_COUNT=$((FAIL_COUNT + 1))
  FAILED_CHECKS+=("$1")
  local detail="${2:-}"
  if [ -n "$detail" ]; then
    printf 'FAIL %s (%s)\n' "$1" "$detail"
  else
    printf 'FAIL %s\n' "$1"
  fi
  if [ "$AUDIT_FAIL_FAST" = "1" ]; then
    printf '\nÉchec immédiat demandé (AUDIT_FAIL_FAST=1).\n' >&2
    exit 1
  fi
}

skip() {
  SKIP_COUNT=$((SKIP_COUNT + 1))
  printf 'skip %s (%s)\n' "$1" "${2:-non applicable}"
}

die() {
  printf 'audit: %s\n' "$1" >&2
  exit 2
}

# Écrit le corps et les en-têtes de la dernière requête dans $AUDIT_TMP.
request() {
  local method="$1"
  local path="$2"
  shift 2
  : >"$AUDIT_TMP/body"
  : >"$AUDIT_TMP/headers"
  HTTP_STATUS="$("${CURL[@]}" -X "$method" -o "$AUDIT_TMP/body" -D "$AUDIT_TMP/headers" -w '%{http_code}' "$@" "$BASE_URL$path" 2>"$AUDIT_TMP/curl.err" || true)"
  if [ -z "$HTTP_STATUS" ]; then
    HTTP_STATUS="000"
  fi
  HTTP_BODY="$AUDIT_TMP/body"
  HTTP_HEADERS="$AUDIT_TMP/headers"
}

session_request() {
  local method="$1"
  local path="$2"
  shift 2
  request "$method" "$path" -b "$SESSION_COOKIE" "$@"
}

bearer_request() {
  local method="$1"
  local path="$2"
  shift 2
  request "$method" "$path" -H "Authorization: Bearer $API_TOKEN" "$@"
}

header_value() {
  local file="$1"
  local name="$2"
  grep -i "^${name}:" "$file" 2>/dev/null | tail -n 1 | sed -E 's/^[^:]+:[[:space:]]*//; s/\r$//'
}

header_present() {
  local file="$1"
  local name="$2"
  grep -qi "^${name}:" "$file" 2>/dev/null
}

expect_status() {
  local name="$1"
  local actual="$2"
  local expected="$3"
  local candidate
  for candidate in $expected; do
    if [ "$actual" = "$candidate" ]; then
      pass "$name"
      return 0
    fi
  done
  fail "$name" "statut $actual, attendu $expected"
  return 0
}

expect_header_contains() {
  local name="$1"
  local header="$2"
  local needle="$3"
  local value
  value="$(header_value "$HTTP_HEADERS" "$header")"
  case "$value" in
    *"$needle"*) pass "$name" ;;
    *) fail "$name" "en-tête $header sans « $needle »" ;;
  esac
}

expect_header_absent() {
  local name="$1"
  local header="$2"
  if header_present "$HTTP_HEADERS" "$header"; then
    fail "$name" "en-tête $header présent"
  else
    pass "$name"
  fi
}

expect_body_contains() {
  local name="$1"
  local needle="$2"
  if grep -Fq -- "$needle" "$HTTP_BODY" 2>/dev/null; then
    pass "$name"
  else
    fail "$name" "corps sans « $needle »"
  fi
}

expect_body_not_contains() {
  local name="$1"
  local needle="$2"
  if grep -Fq -- "$needle" "$HTTP_BODY" 2>/dev/null; then
    fail "$name" "corps contient « $needle »"
  else
    pass "$name"
  fi
}

expect_body_matches() {
  local name="$1"
  local pattern="$2"
  if grep -Eq -- "$pattern" "$HTTP_BODY" 2>/dev/null; then
    pass "$name"
  else
    fail "$name" "corps sans motif attendu"
  fi
}

expect_header_matches() {
  local name="$1"
  local header="$2"
  local pattern="$3"
  local value
  value="$(header_value "$HTTP_HEADERS" "$header")"
  if printf '%s' "$value" | grep -Eq -- "$pattern"; then
    pass "$name"
  else
    fail "$name" "en-tête $header non conforme"
  fi
}

# Vérifie qu'une réponse JSON porte le code d'erreur attendu.
expect_json_code() {
  local name="$1"
  local code="$2"
  expect_header_contains "$name (content-type)" "content-type" "application/json" >/dev/null
  expect_body_contains "$name" "\"code\":\"$code\""
}

json_string() {
  local key="$1"
  sed -n "s/.*\"${key}\":\"\\([^\"]*\\)\".*/\\1/p" "$HTTP_BODY" | head -n 1
}

json_number() {
  local key="$1"
  sed -n "s/.*\"${key}\":\\([0-9][0-9]*\\).*/\\1/p" "$HTTP_BODY" | head -n 1
}

# Connexion de l'utilisateur d'audit ; le jeton de session reste en variable, jamais affiché.
login_session() {
  request POST "/login" \
    -H "Content-Type: application/x-www-form-urlencoded" \
    -H "Origin: ${BASE_URL%/}" \
    --data-urlencode "username=$AUDIT_USER" \
    --data-urlencode "password=$AUDIT_PASS"
  if [ "$HTTP_STATUS" != "303" ]; then
    return 1
  fi
  LAST_SET_COOKIE="$(header_value "$HTTP_HEADERS" set-cookie)"
  local token="${LAST_SET_COOKIE#fs-ez_session=}"
  token="${token%%;*}"
  if [ -z "$token" ] || [ "$token" = "$LAST_SET_COOKIE" ]; then
    return 1
  fi
  SESSION_COOKIE="fs-ez_session=$token"
  return 0
}

# Contrôle de sécurité des en-têtes sur la dernière réponse.
check_security_headers() {
  local label="$1"
  expect_header_contains "headers: CSP script-src $label" "content-security-policy" "script-src 'self'"
  expect_header_contains "headers: CSP frame-ancestors $label" "content-security-policy" "frame-ancestors 'none'"
  expect_header_contains "headers: CSP base-uri $label" "content-security-policy" "base-uri 'none'"
  expect_header_contains "headers: CSP form-action $label" "content-security-policy" "form-action 'self'"
  expect_header_contains "headers: nosniff $label" "x-content-type-options" "nosniff"
  expect_header_contains "headers: X-Frame-Options $label" "x-frame-options" "DENY"
  expect_header_matches "headers: Referrer-Policy $label" "referrer-policy" "."
  expect_header_matches "headers: Permissions-Policy $label" "permissions-policy" "."
  expect_header_matches "headers: HSTS $label" "strict-transport-security" "^max-age=[1-9][0-9]*"
  expect_header_absent "headers: Server absent $label" "server"
  expect_header_absent "headers: X-Powered-By absent $label" "x-powered-by"
}

# Cherche un script non externe, un handler inline ou une URL javascript:.
# Les valeurs d'attributs sont retirées avant l'analyse : un nom échappé ne devient jamais un handler.
check_inline_html() {
  local label="$1"
  local stripped="$AUDIT_TMP/html-sans-values"
  sed -E "s/=\"[^\"]*\"//g; s/='[^']*'//g" "$HTTP_BODY" >"$stripped" 2>/dev/null || : >"$stripped"
  local bad
  bad="$(grep -Eoi '<script[^>]*>' "$HTTP_BODY" 2>/dev/null | grep -Evi '^<script[[:space:]]+src="/static/' || true)"
  if [ -z "$bad" ]; then
    pass "html: scripts externes uniquement $label"
  else
    fail "html: scripts externes uniquement $label"
  fi
  if grep -Eqi '<[a-z][^>]*[[:space:]]on[a-z]+[[:space:]]*=' "$stripped" 2>/dev/null; then
    fail "html: aucun on*= $label"
  else
    pass "html: aucun on*= $label"
  fi
  if grep -Eqi '<[a-z][^>]*[[:space:]]style[[:space:]]*=' "$stripped" 2>/dev/null \
    || grep -Fqi '<style' "$stripped" 2>/dev/null; then
    fail "html: aucun style inline $label"
  else
    pass "html: aucun style inline $label"
  fi
  if grep -Eqi '(href|src|action)[[:space:]]*=[[:space:]]*["'\'']?[[:space:]]*javascript:' "$HTTP_BODY" 2>/dev/null; then
    fail "html: aucune URL javascript: $label"
  else
    pass "html: aucune URL javascript: $label"
  fi
}

file_mode() {
  stat -c '%a' "$1" 2>/dev/null || true
}

# Aucune réponse d'erreur ne doit exposer chemin serveur, SQL, hash ou backtrace.
audit_no_leak() {
  local label="$1"
  local pattern='(/home/|/tmp/|crates/fs-ez|SELECT |INSERT |UPDATE |DELETE FROM|rusqlite|sqlite3|argon2|\$argon2|backtrace|panicked at)'
  if grep -Eqi -- "$pattern" "$HTTP_BODY" 2>/dev/null; then
    fail "$label"
  else
    pass "$label"
  fi
}
