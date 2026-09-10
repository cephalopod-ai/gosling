---
sidebar_position: 90
title: Running a Separate Local gosling Server
sidebar_label: Local External Server
---

# Running a Separate Local gosling Server

gosling Desktop normally launches and owns a `gosling serve` child process. Advanced local setups
can run that ACP backend separately and point Desktop at its HTTP(S) base URL. The old standalone
`goslingd` binary and duplicate `gosling-server` REST API were removed in v1.2.5; they are not part
of this setup.

This server is a single-operator control plane. Keep it on a loopback address unless you have
designed a separate network and authentication boundary around it. Never expose
`--dangerously-unauthenticated` beyond loopback; gosling rejects that combination.

## Start the ACP server

Choose a long random secret and use the same value for the server and Desktop:

```bash
export GOSLING_SERVER__SECRET_KEY='YOUR_LONG_RANDOM_SECRET'
gosling serve --host 127.0.0.1 --port 3284 --platform desktop
```

The supported command and security controls are:

| Option | Purpose |
|---|---|
| `--host` | Address to bind; defaults to `127.0.0.1`. |
| `--port` | TCP port; defaults to `3284`. |
| `--platform desktop` | Identifies requests as coming from gosling Desktop. |
| `--tls` | Serves ACP over TLS. Without certificate paths, gosling creates or reuses a local self-signed certificate. |
| `--tls-cert-path` and `--tls-key-path` | Use a specific PEM certificate and private key. Both are required together. |
| `GOSLING_SERVER__SECRET_KEY` | Requires the matching token on status and ACP connections. |
| `--allowed-origin` | Replaces the default loopback CORS origins with one or more exact origins. Wildcards are rejected. |

`gosling serve` refuses to start without `GOSLING_SERVER__SECRET_KEY` unless
`--dangerously-unauthenticated` is present. The unauthenticated mode is for deliberate loopback
development only and cannot bind to a non-loopback address.

### Optional TLS

For a self-signed local certificate:

```bash
export GOSLING_SERVER__SECRET_KEY='YOUR_LONG_RANDOM_SECRET'
gosling serve --host 127.0.0.1 --port 3284 --platform desktop --tls
```

At startup, the server prints `GOSLINGD_CERT_FINGERPRINT=...`. The variable name is retained in the
log format for compatibility even though the `goslingd` executable no longer exists. Copy that
fingerprint if you want Desktop to pin the exact certificate. If no fingerprint is configured,
Desktop trusts the first certificate it sees for that external HTTPS backend and pins it for the
life of the backend registration (TOFU).

## Verify the listener

Confirm that the process is listening:

```bash
lsof -nP -iTCP:3284 -sTCP:LISTEN
```

Then check the authenticated status route. Add `-k` only when using the generated self-signed TLS
certificate:

```bash
curl -i http://127.0.0.1:3284/status \
  -H 'X-Secret-Key: YOUR_LONG_RANDOM_SECRET'
```

For TLS, use `https://127.0.0.1:3284/status -k`. A successful status response proves that the
listener and shared secret are working. Desktop performs a second authenticated ACP probe before
opening a chat.

## Configure Desktop

Open **Settings → External Backend (ACP)** and set:

| Setting | Value |
|---|---|
| **Use external backend** | Enabled |
| **Backend Base URL** | `http://127.0.0.1:3284` or the matching `https://` URL |
| **Secret Key** | The server's `GOSLING_SERVER__SECRET_KEY` value |
| **Certificate Fingerprint** | Optional; HTTPS only |

Enter the base URL before `/acp`, without query parameters or a fragment. The secret is held only
for the current app launch and is intentionally not persisted, so enter it again after restarting
Desktop. Setting changes apply to new chat windows; restart gosling to update existing windows.

Desktop can also be launched with an explicit external backend environment:

```bash
export GOSLING_EXTERNAL_BACKEND=true
export GOSLING_EXTERNAL_BACKEND_URL='http://127.0.0.1:3284'
export GOSLING_SERVER__SECRET_KEY='YOUR_LONG_RANDOM_SECRET'
```

Set `GOSLING_EXTERNAL_BACKEND_URL` explicitly. The legacy fallback used when that variable is
omitted still points to port `3000`, while `gosling serve` now defaults to `3284`.

## Troubleshooting

- **Unauthorized or unreachable:** confirm the base URL, protocol, port, and shared secret. Desktop
  sends the secret to `/status` and uses it as the ACP connection token.
- **Certificate error:** configure the startup fingerprint, or remove an obsolete fingerprint and
  allow a new trust-on-first-use registration. Fingerprints require an `https://` base URL.
- **Existing windows use the old backend:** open a new chat window or restart Desktop.
- **Custom web origin is blocked:** pass each exact origin with `--allowed-origin`; do not use a
  wildcard.

## Related

- [Environment Variables](/docs/guides/environment-variables)
- [Configuration Files](/docs/guides/config-files)
- [ACP Providers](/docs/guides/acp-providers)
