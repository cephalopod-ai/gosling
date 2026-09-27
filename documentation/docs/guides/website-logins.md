---
title: Website logins
sidebar_position: 32
description: Save website credentials for approved browser tool calls without exposing passwords to the model or transcript
---

# Website logins

Website logins let gosling sign in through a browser-capable tool without sending the saved password
to the model. The agent can see the login's name, website, username, and a placeholder such as
`{{login:Work GitHub}}`; gosling replaces that placeholder only when it dispatches an approved tool
call.

:::info Desktop only
Website-login management is currently available under **Settings → Credentials** in gosling
Desktop.
:::

## Save a login

1. Open **Settings → Credentials → Website Logins**.
2. Select **Add login**.
3. Enter a unique name, the full `http://` or `https://` website URL, the username or email, and the
   password.
4. Select **Save**.

The name is how the agent refers to the login. It may contain letters, numbers, spaces, `_`, `-`,
`.` or `@`. When editing an existing login, leave the password blank to keep its current value.
Deleting a login removes both its metadata and saved password.

## Use a login

Ask gosling to sign in to the saved website and identify the account by name. The built-in
`website_logins` tool lists the non-secret account details and password placeholder. When the agent
passes that placeholder to another tool, gosling:

1. checks that the tool call's website origin matches the saved login;
2. asks for approval unless the session is in Auto mode;
3. substitutes the password immediately before dispatch; and
4. replaces the password with its placeholder in the tool result and progress notifications.

:::warning Match the saved website
A password placeholder is restricted to the saved website's origin. Save the actual sign-in URL,
and review the destination shown in the approval prompt before allowing the call.
:::

## Security boundary

- Passwords use gosling's secret store; login names, URLs, and usernames use ordinary
  configuration.
- The model and conversation transcript receive the placeholder, not the stored password.
- Substitution applies to a single approved tool call. gosling redacts that password from the
  call's returned content, including structured values and progress notifications.
- Auto mode skips the confirmation prompt. Use an approval-based mode when you want to review each
  credential use.
- Redaction protects gosling's tool-output path; it cannot control what the destination website or
  an external browser tool records after receiving the credential.

If a password is missing, edit the login and enter a replacement. gosling will not dispatch a
placeholder for a login that has no saved password.
