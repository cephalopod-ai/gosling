#!/usr/bin/env bash

set -euo pipefail

REPOSITORY_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

assert_contains() {
    local FILE_PATH="$1"
    local EXPECTED="$2"
    local CONTRACT="$3"

    if ! grep -Fq -- "$EXPECTED" "${REPOSITORY_ROOT}/${FILE_PATH}"; then
        echo "Phase 1 integrity check failed (${CONTRACT}): ${FILE_PATH} is missing: ${EXPECTED}" >&2
        exit 1
    fi
}

assert_not_contains() {
    local FILE_PATH="$1"
    local FORBIDDEN="$2"
    local CONTRACT="$3"

    if grep -Fq -- "$FORBIDDEN" "${REPOSITORY_ROOT}/${FILE_PATH}"; then
        echo "Phase 1 integrity check failed (${CONTRACT}): ${FILE_PATH} contains: ${FORBIDDEN}" >&2
        exit 1
    fi
}

JBANG_SHIM="ui/desktop/src/bin/jbang"
SMOKE_WORKFLOW=".github/workflows/pr-smoke-test.yml"
BUILD_WORKFLOW=".github/workflows/build-cli.yml"
DOCS_WORKFLOW=".github/workflows/docs-update-cli-ref.yml"
WORKSPACE_MANIFEST="ui/package.json"
PNPM_LOCK="ui/pnpm-lock.yaml"

assert_not_contains "$JBANG_SHIM" "sh.jbang.dev" "SEC-REQ-001"
assert_contains "$JBANG_SHIM" 'JBANG_VERSION="0.139.3"' "SEC-REQ-001"
assert_contains "$JBANG_SHIM" 'JBANG_SHA256="6ff8d2f387583a8b1b1eb7839826a5e0a227c7cf1550e3bd85e0beb4838ca3ef"' "SEC-REQ-001"
assert_contains "$JBANG_SHIM" 'releases/download/v${JBANG_VERSION}/jbang-${JBANG_VERSION}.zip' "SEC-REQ-001"
assert_contains "$JBANG_SHIM" 'verify_sha256 "${JBANG_SHA256}" "${JBANG_ARCHIVE}"' "SEC-REQ-001"

assert_not_contains "$SMOKE_WORKFLOW" "npm install -g" "SEC-REQ-002"
assert_not_contains "$SMOKE_WORKFLOW" "@zed-industries/claude-agent-acp" "SEC-REQ-002"
assert_not_contains "$SMOKE_WORKFLOW" "@zed-industries/codex-acp" "SEC-REQ-002"
assert_contains "$WORKSPACE_MANIFEST" '"@anthropic-ai/claude-code": "2.1.206"' "SEC-REQ-002"
assert_contains "$WORKSPACE_MANIFEST" '"@agentclientprotocol/claude-agent-acp": "0.58.1"' "SEC-REQ-002"
assert_contains "$WORKSPACE_MANIFEST" '"@agentclientprotocol/codex-acp": "1.1.2"' "SEC-REQ-002"
assert_contains "$WORKSPACE_MANIFEST" '"@modelcontextprotocol/sdk": "1.29.0"' "SEC-REQ-002"
assert_contains "$WORKSPACE_MANIFEST" '"zod": "4.4.3"' "SEC-REQ-002"
assert_contains "$WORKSPACE_MANIFEST" '"@agentclientprotocol/claude-agent-acp>zod": "4.4.3"' "SEC-REQ-002"
assert_contains "$PNPM_LOCK" "'@anthropic-ai/claude-code':" "SEC-REQ-002"
assert_contains "$PNPM_LOCK" "'@agentclientprotocol/claude-agent-acp':" "SEC-REQ-002"
assert_contains "$PNPM_LOCK" "'@agentclientprotocol/codex-acp':" "SEC-REQ-002"
assert_contains "$PNPM_LOCK" 'sha512-THRCtnNNX5usIEdf4ulEv31Fq/WL55sD5u8Y6rQLTEmLkwAKdHo1FhfWzfHI3ur/gJ0SU5x4irpvY7VAIXX24w==' "SEC-REQ-002"
assert_contains "$PNPM_LOCK" 'sha512-F1/W6EJdoYbrEUluRUknx0Nn0MAKDOkn2C/9YcP/joVkmdFUGTAxlGDpwdYu239TOkpc8Qm4+ffGsQjPZdryTg==' "SEC-REQ-002"
assert_contains "$PNPM_LOCK" 'sha512-qE/R1WdqJJ9OFHsHGvbmVmS2j9iCMZzpWT3g2XIViXrGHu1fLOALLINBIlW+WzKDllCh131aB6cqcIWSt0otbw==' "SEC-REQ-002"

assert_not_contains "$BUILD_WORKFLOW" "sh.rustup.rs" "SEC-REQ-003"
assert_contains "$BUILD_WORKFLOW" 'RUSTUP_VERSION="1.29.0"' "SEC-REQ-003"
assert_contains "$BUILD_WORKFLOW" '4acc9acc76d5079515b46346a485974457b5a79893cfb01112423c89aeb5aa10' "SEC-REQ-003"
assert_contains "$BUILD_WORKFLOW" '9732d6c5e2a098d3521fca8145d826ae0aaa067ef2385ead08e6feac88fa5792' "SEC-REQ-003"
assert_contains "$BUILD_WORKFLOW" 'rustup/archive/${RUSTUP_VERSION}/${TARGET}/rustup-init' "SEC-REQ-003"
assert_contains "$BUILD_WORKFLOW" 'sha256sum -c -' "SEC-REQ-003"

assert_not_contains "$DOCS_WORKFLOW" "releases/download/stable/download_cli.sh" "SEC-REQ-004"
assert_contains "$DOCS_WORKFLOW" "toolchain: '1.92'" "SEC-REQ-004"
assert_contains "$DOCS_WORKFLOW" 'cargo build --locked -p gosling-cli --bin gosling' "SEC-REQ-004"
assert_contains "$DOCS_WORKFLOW" 'install -m 0755 target/debug/gosling /home/runner/.local/bin/gosling' "SEC-REQ-004"
assert_contains "$DOCS_WORKFLOW" '/home/runner/.local/bin/gosling --version' "SEC-REQ-004"

echo "Phase 1 integrity contracts passed."
