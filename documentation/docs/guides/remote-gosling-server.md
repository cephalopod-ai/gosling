---
sidebar_position: 90
title: Running a Separate Local gosling Server
sidebar_label: Local External Server
---

# Running a Separate Local gosling Server

gosling Desktop normally starts its own `goslingd` process. Advanced local setups may start that process separately and connect Desktop to it through a loopback address.

`goslingd` is a single-operator local control plane. It does not support binding to a LAN, VPN, public, wildcard, or other non-loopback address. `GOSLING_HOST` must be a numeric loopback address such as `127.0.0.1` or `::1`. Use a separately designed multi-user service instead of exposing `goslingd` remotely.

## Initial Setup

### 1. Start the `goslingd` server

On the same machine as Desktop, launch `goslingd` with a loopback host, port, TLS, and a secret key:

```bash
GOSLING_HOST=127.0.0.1 \
GOSLING_PORT=3000 \
GOSLING_TLS=true \
GOSLING_SERVER__SECRET_KEY='YOUR_SECRET' \
/Applications/Gosling.app/Contents/Resources/bin/goslingd agent
```

On Linux or Windows the path to the binary differs. Use the binary bundled with your gosling installation or a standalone local build.

| Variable | Purpose |
|----------|---------|
| `GOSLING_HOST` | Numeric loopback address. Use `127.0.0.1` or `::1`; non-loopback addresses are rejected. |
| `GOSLING_PORT` | TCP port to listen on. |
| `GOSLING_TLS` | Must be `true`. gosling Desktop will not connect to a plain HTTP server. |
| `GOSLING_SERVER__SECRET_KEY` | Shared secret. The client must send this in the `X-Secret-Key` header. Treat it like a password. |

:::tip
Pick a long, random value for `GOSLING_SERVER__SECRET_KEY` and store it in a password manager — the same value goes into gosling Desktop later.
:::

### 2. Verify the server is up

First, confirm `goslingd` is actually listening on the port you expect:

```bash
lsof -nP -iTCP:3000 -sTCP:LISTEN
```

Then test the endpoints from the server itself. The `-k` flag tells `curl` to accept the self-signed TLS certificate that `goslingd` generates:

```bash
# Connectivity only
curl -i https://127.0.0.1:3000/status -k

# Authenticated endpoint (real test)
curl -i https://127.0.0.1:3000/config/read -k \
  -H 'Content-Type: application/json' \
  -H 'X-Secret-Key: YOUR_SECRET' \
  --data '{"key":"GOSLING_PROVIDER","is_secret":false}'
```

A `200` response from the second call confirms that TLS is up, the secret key is being accepted, and the server is ready to receive client requests.

### 3. Find the certificate fingerprint

Because `goslingd` generates a self-signed TLS certificate, gosling Desktop pins it by SHA-256 fingerprint rather than relying on a public certificate authority.

When TLS is enabled, `goslingd` logs the fingerprint on startup. It looks like:

```text
GOSLINGD_CERT_FINGERPRINT=AA:BB:CC:DD:EE:FF:...
```

To capture it, either:

- Run `goslingd` interactively and read it from the terminal output, or
- Tail the log file you redirect to when running as a service (see [Running `goslingd` as a background service](#running-goslingd-as-a-background-service-macos)):

```bash
grep GOSLINGD_CERT_FINGERPRINT ~/Library/Logs/GoslingExternal/goslingd.out.log
```

Make a note of the fingerprint — you will paste it into gosling Desktop in the next step.

:::note
The fingerprint changes whenever `goslingd` regenerates its certificate (for example, if you delete the cert file). If gosling Desktop suddenly refuses to connect after a server restart, re-check the fingerprint.
:::

### 4. Configure gosling Desktop

On the client machine, open gosling Desktop and navigate to **Settings → gosling Server**:

| Setting | Value |
|---------|-------|
| **Use external server** | Enabled |
| **URL** | `https://127.0.0.1:3000` |
| **Secret Key** | The same value you used for `GOSLING_SERVER__SECRET_KEY` |
| **Certificate Fingerprint** | The `GOSLINGD_CERT_FINGERPRINT` value from the server logs |

After saving, gosling Desktop routes backend requests to the separately managed local `goslingd`.

## Running `goslingd` as a Background Service (macOS)

Running `goslingd` in a terminal session is fine for testing, but for everyday use you probably want it managed as a background service so it starts at login and restarts on failure. On macOS, this is done with `launchd`.

Create a LaunchAgent plist at `~/Library/LaunchAgents/com.gosling.goslingd.external.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key>
    <string>com.gosling.goslingd.external</string>

    <key>ProgramArguments</key>
    <array>
      <string>/Applications/Gosling.app/Contents/Resources/bin/goslingd</string>
      <string>agent</string>
    </array>

    <key>EnvironmentVariables</key>
    <dict>
      <key>GOSLING_HOST</key><string>127.0.0.1</string>
      <key>GOSLING_PORT</key><string>3000</string>
      <key>GOSLING_TLS</key><string>true</string>
      <key>GOSLING_SERVER__SECRET_KEY</key><string>YOUR_SECRET</string>
    </dict>

    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>

    <key>StandardOutPath</key>
    <string>/Users/YOUR_USERNAME/Library/Logs/GoslingExternal/goslingd.out.log</string>
    <key>StandardErrorPath</key>
    <string>/Users/YOUR_USERNAME/Library/Logs/GoslingExternal/goslingd.err.log</string>
  </dict>
</plist>
```

Replace `YOUR_SECRET` and `YOUR_USERNAME` with appropriate values, and make sure the log directory exists:

```bash
mkdir -p ~/Library/Logs/GoslingExternal
```

Then load and start the service:

```bash
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.gosling.goslingd.external.plist
launchctl kickstart -k gui/$(id -u)/com.gosling.goslingd.external
```

To stop or remove it later:

```bash
launchctl bootout gui/$(id -u)/com.gosling.goslingd.external
```

:::tip
Because the secret key is stored in plain text in the plist, the file should be readable only by your user. macOS LaunchAgents under `~/Library/LaunchAgents/` are already user-scoped, but you can tighten further with `chmod 600 ~/Library/LaunchAgents/com.gosling.goslingd.external.plist`.
:::

## Troubleshooting

### Server rejects the configured host

`goslingd` intentionally rejects non-loopback and wildcard bind addresses. Set `GOSLING_HOST=127.0.0.1` or `GOSLING_HOST=::1`, then restart it. Verify the listener with:

```bash
lsof -nP -iTCP:3000 -sTCP:LISTEN
```

The output should show a loopback listener, not `*:3000` or an external address.

### TLS is not enabled

In the server's startup logs:

- If you see `listening on http://...`, TLS is **not** enabled. gosling Desktop will not connect. Set `GOSLING_TLS=true` and restart `goslingd`.
- If you see `listening on https://...`, TLS is enabled and you are good to go.

The startup logs also contain the `GOSLINGD_CERT_FINGERPRINT=...` line you need for the gosling Desktop configuration. Search the server's stdout (or log file, if running under `launchd`) for `GOSLINGD_CERT_FINGERPRINT` to find it.

### Client cannot authenticate (401 / Unauthorized)

A `401` from the server, or a gosling Desktop error indicating that the secret was rejected, almost always means that `GOSLING_SERVER__SECRET_KEY` on the server does not match the **Secret Key** in gosling Desktop's settings.

To check the secret end-to-end without involving gosling Desktop, run the authenticated `curl` from [step 2](#2-verify-the-server-is-up) using exactly the value you have configured on the client. If that returns `200`, the secret is correct and the problem is in the client configuration; if it returns `401`, the secret on the server is different from what you are sending.

If you rotate the secret on the server, you must also update it in gosling Desktop's settings — they are not synchronized automatically.

### Certificate fingerprint mismatch

If gosling Desktop refuses to connect with a certificate or fingerprint error, the most common causes are:

- The server regenerated its certificate (for example, after deleting the cert file). Look at the latest startup logs for the current `GOSLINGD_CERT_FINGERPRINT` and update gosling Desktop.
- You copied the fingerprint with extra whitespace or pasted the wrong value.

## Related

- [Environment Variables](/docs/guides/environment-variables) — full reference for all `GOSLING_*` variables
- [Configuration Files](/docs/guides/config-files) — persistent client-side configuration
