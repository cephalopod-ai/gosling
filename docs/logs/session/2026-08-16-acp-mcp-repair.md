# 2026-08-16 — ACP / MCP repair batch

**Branch:** `repair/acp-mcp-2026-08-16` off `main` @ `3c2f899f5`
**Skill:** `repair-defect-campaign` (governed_repair)
**Scope:** open ACP/MCP-related findings from the 2026-08-15 audit.

## Inventory (ACP/MCP findings still open at start)

| ID | Sev | Disposition this run |
|---|---|---|
| SEC-GOS-002 | High | **Fixed** — `5ea594f4b` |
| SEC-GOS-007 | Medium (High if `GOSLING_HOST` public) | **Assessed, not patched** — see below |
| SECN-GSL-001 | High | **Open** — needs the token to replace the secret |
| CAS-GSL-003, CON-GSL-003, MCP-GOS-001, ARC-GSL-004, IOP-GOS-004 | Medium | Not started |

Already closed in earlier batches: SEC-GOS-001, SEC-GOS-008, REL-GSL-005,
ARCN-GSL-001, RES-GSL-002.

## Fixed — SEC-GOS-002 (`5ea594f4b`)

The live ACP path let the guest CSP be supplied by the client: the proxy page
read its own `<meta>` policy, rewrote `'nonce-...'` to `'unsafe-inline'` in
`createGuestCsp`, and POSTed the string to `/mcp-app-guest`, which installed it
verbatim. The policy bounding the guest frame was authored by that frame.

`/mcp-app-proxy` now derives the guest policy at render time from the same
declared domains it already uses for the outer policy, stores it under a
single-use token, and injects only the token. `/mcp-app-guest` consumes the
token; `csp` is gone from the body and `createGuestCsp` from the template. An
unknown or expired token is refused rather than defaulting to no CSP.

Verified live against a running `gosling serve`:

| case | result |
|---|---|
| proxy page render | carries `proxyToken: '<server-minted>'`; `createGuestCsp` absent |
| POST forged token + `"csp":"default-src *"` | **400**, policy ignored |
| POST minted token | 200, guest stored |
| GET guest URL | `default-src 'none'; … connect-src 'self' https://api.example.com; …` |

## Assessed, not patched — SEC-GOS-007

Filed as "goslingd leaves MCP-app routes unauthenticated". Reading the actual
routes, the exposure is narrower than the title:

- `POST /mcp-app-guest` — the only state-changing route — **is** authenticated
  in-handler via `token_matches(body.secret, state.secret_key)`, and already
  derives its CSP server-side.
- `GET /mcp-app-guest?nonce=` — UUIDv4, single-use, TTL'd. Not guessable.
- `GET /mcp-app-proxy` — returns a static shell. The secret reaches that page
  through the URL *fragment*, which browsers never send to the server, so the
  response carries no secret or session data.

The residual unauthenticated exposure is therefore the proxy shell and the
declared domains echoed into its CSP. Forcing loopback on these routes (what
the live ACP path does) would require switching goslingd from
`into_make_service()` to `into_make_service_with_connect_info` and would break
any legitimate remote deployment that uses MCP apps — a real cost for a
low-value gain, given the state-changing route is already authenticated. Not
patched; recorded here so the reasoning survives rather than being rediscovered.

## Open — SECN-GSL-001, and why it is the right next step

Both templates (`crates/gosling/src/acp/templates/mcp_app_proxy.html:37` and
`crates/gosling-server/src/routes/templates/mcp_app_proxy.html:44`) read the
backend secret from `window.location.hash` in a frame that runs under
`allow-scripts allow-same-origin`. Anything executing there can read the
fragment and exfiltrate the secret.

The `proxy_token` introduced by the SEC-GOS-002 fix is the mechanism that
should replace it: the token is already server-minted, single-use, and scoped
to one proxy render. Making `store_guest_html` accept the token *instead of*
the secret would remove the backend secret from the browser entirely.

That change spans both Rust paths, both templates, and Desktop's
`main.ts` (which builds `proxyUrl.hash`), so it was not attempted at the tail
of this batch. It is the highest-value remaining ACP/MCP item.

## Validation

`cargo clippy --all-targets` 0 issues; gosling lib 1676 passed / 1 pre-existing
failure (`context_mgmt::summarizer::tests::defaults_to_off`), including three
new tests; gosling-server 36 passed.
