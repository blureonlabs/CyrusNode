#!/usr/bin/env bash
# check-domain-imports.sh
#
# Enforces the Apollo dependency rule on src/features/*/domain/.
#
# Domain code is pure. It must not import I/O surfaces (sqlx, reqwest, axum,
# tokio::{net,fs}), platform I/O modules (db, llm, crawler, events, queue,
# prompts), or cross-feature modules. The only platform module domain may use
# is `platform::core`.
#
# Exits 1 on any violation; 0 if the tree is clean.

set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

FEATURES_ROOT="src/features"
if [ ! -d "${FEATURES_ROOT}" ]; then
  echo "✓ domain-import rule clean (no src/features/ tree yet)"
  exit 0
fi

# Rules use TAB as the delimiter between pattern and description (TABs cannot
# appear inside our extended-regex patterns).
TAB=$'\t'
RULES=(
  "(^|[^[:alnum:]_])sqlx(::|;)${TAB}sqlx forbidden in domain (use a port + infra adapter)"
  "(^|[^[:alnum:]_])reqwest(::|;)${TAB}reqwest forbidden in domain (use a port + infra adapter)"
  "(^|[^[:alnum:]_])axum(::|;)${TAB}axum forbidden in domain (presentation/infra only)"
  "(^|[^[:alnum:]_])tokio::net(::|;)${TAB}tokio::net forbidden in domain"
  "(^|[^[:alnum:]_])tokio::fs(::|;)${TAB}tokio::fs forbidden in domain"
  "(^|[^[:alnum:]_])crate::platform::db(::|;)${TAB}crate::platform::db forbidden in domain"
  "(^|[^[:alnum:]_])crate::platform::llm(::|;)${TAB}crate::platform::llm forbidden in domain"
  "(^|[^[:alnum:]_])crate::platform::crawler(::|;)${TAB}crate::platform::crawler forbidden in domain"
  "(^|[^[:alnum:]_])crate::platform::events(::|;)${TAB}crate::platform::events forbidden in domain"
  "(^|[^[:alnum:]_])crate::platform::queue(::|;)${TAB}crate::platform::queue forbidden in domain"
  "(^|[^[:alnum:]_])crate::platform::prompts(::|;)${TAB}crate::platform::prompts forbidden in domain"
  "(^|[^[:alnum:]_])crate::features::${TAB}cross-feature use forbidden in domain (talk via events or platform::core)"
)

VIOLATIONS=0
TMP_OUTPUT="$(mktemp -t apollo-domain-check.XXXXXX)"
trap 'rm -f "${TMP_OUTPUT}"' EXIT

# Discover every domain directory under src/features/*/domain.
DOMAIN_DIRS=()
while IFS= read -r d; do
  DOMAIN_DIRS+=("$d")
done < <(find "${FEATURES_ROOT}" -mindepth 2 -maxdepth 2 -type d -name domain 2>/dev/null | sort)

if [ "${#DOMAIN_DIRS[@]}" -eq 0 ]; then
  echo "✓ domain-import rule clean (no src/features/*/domain/ directories yet)"
  exit 0
fi

for rule in "${RULES[@]}"; do
  pattern="${rule%%${TAB}*}"
  description="${rule#*${TAB}}"

  for dir in "${DOMAIN_DIRS[@]}"; do
    if grep -rEnH --include='*.rs' "${pattern}" "${dir}" >"${TMP_OUTPUT}" 2>/dev/null; then
      while IFS= read -r match; do
        file="${match%%:*}"
        rest="${match#*:}"
        line="${rest%%:*}"
        content="${rest#*:}"
        trimmed="$(printf '%s' "${content}" | sed -E 's/^[[:space:]]+//')"
        # Skip comment-only lines (// /// //! /* etc). Comments may legitimately
        # mention forbidden symbols when documenting the rule itself.
        case "${trimmed}" in
          //*|\*/*|/\**) continue ;;
        esac
        echo "✗ ${file}:${line}"
        echo "    ${trimmed}"
        echo "    rule: ${description}"
        VIOLATIONS=$((VIOLATIONS + 1))
      done <"${TMP_OUTPUT}"
    fi
  done
done

if [ "${VIOLATIONS}" -gt 0 ]; then
  echo ""
  echo "✗ domain-import rule violated: ${VIOLATIONS} offending line(s)."
  echo "  Domain code must be pure. Move I/O behind a port (domain/ports.rs) and"
  echo "  implement it in infra/. See apollo/02-architecture/feature-layout.md."
  exit 1
fi

echo "✓ domain-import rule clean"
exit 0
