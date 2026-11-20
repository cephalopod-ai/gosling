---
title: Dedicated email
---

Use a dedicated mailbox for gosling Desktop to read and organize. While Desktop is running,
gosling checks the inbox about once a minute. You can also process mail from Settings.

## Set up the mailbox

1. Open **Settings → Auth → Email**. Choose **Purelymail**, **Gmail**, or **Custom** and enter
   the dedicated address. Use a mailbox or app password for Purelymail and a Google app password
   for an eligible Gmail account.
2. Use **IMAP** for server folders and Trash. **POP3** supports local folders and local Trash.
   Check the provider's POP settings for how it treats downloaded messages. gosling leaves the
   POP server copy in place.
3. Select **Choose email model** to pick a configured direct provider and model. This selection
   applies to the mailbox. The model receives the sender, subject, and bounded message text
   needed to classify the mail.
   A remote API provider receives this content at its endpoint. Use a local Ollama instance
   to keep classification on this computer. For a provider that uses an API key, save its
   credentials under **Settings → Auth** so the email worker can use them.
4. Select **Instructions** to set how gosling should sort mail and draft replies. Use **Advanced**
   to configure filing folders. Add addresses under **Receive allowlist** for senders gosling
   may process and under **Send allowlist** for recipients it may email. Enter one address per
   line. Add an address to both lists when you want gosling to process its mail and reply to it.
5. Select **Save email settings**, then **Test connection**, then **Enable email**. The
   connection test signs in to the incoming and outgoing servers separately. A verified
   incoming connection is enough to enable inbox processing. Sending replies also requires a
   successful SMTP test. **Process now** triggers
   an inbox check. On the first check, gosling starts with new mail unless you selected
   **Process existing mail on the first inbox check** when setting up the account.

After changing the email model on an active mailbox, select **Enable email** to resume processing.

Passwords are stored in the operating system credential store. If the store is unavailable,
gosling cannot save or use the mailbox password.

| Preset | IMAP | POP3 | SMTP |
| --- | --- | --- | --- |
| Purelymail | `imap.purelymail.com:993` | `pop3.purelymail.com:995` | `smtp.purelymail.com:465` |
| Gmail | `imap.gmail.com:993` | `pop.gmail.com:995` | `smtp.gmail.com:465` |

The presets use TLS. Edit **Server settings** to use port 587 with STARTTLS for SMTP or to
enter custom servers, ports, and logins. Check your Google account's app-password eligibility.
If you select Gmail POP3, enable POP in Gmail. [Purelymail server settings](https://support.purelymail.com/support/solutions/articles/159000430778-server-settings-imap-smtp-pop3-and-managesieve-protocols) · [Gmail server settings](https://developers.google.com/workspace/gmail/imap/imap-smtp) · [Google app passwords](https://support.google.com/accounts/answer/185833) · [Gmail POP setup](https://support.google.com/mail/answer/7104828)

## Review decisions and replies

Open **Mail activity** to see what happened. gosling can keep mail, move it to an existing
configured folder, archive it, retain a local text copy, move it to Trash, or draft a reply.
On IMAP, the server must support a safe move to use its folders. Use **Restore** where
available, or restore from the provider's webmail while the message is still in Trash.

gosling matches the message's `From` address against the **Receive allowlist** before sending
any message content to the model or acting on it. Mail from other addresses stays in the inbox.
An empty **Receive allowlist** stops processing. Existing accounts keep their **Send allowlist**
when upgrading and begin with an empty **Receive allowlist**.

When the model proposes a reply to an accepted message, gosling can send it automatically if
SMTP is verified and the `From` address is on the **Send allowlist**. The reply comes from the
dedicated mailbox. gosling ignores `Reply-To` and copied recipients. If SMTP is unavailable,
eligible replies remain drafts in **Mail activity**. An empty **Send allowlist** blocks all sends.

Use **Disable email** to stop processing. Use **Disconnect** to
require a new connection test. **Forget credentials** removes the saved passwords. Mail
activity, drafts, and local copies stay on this computer until you discard them under
**Mail activity**. If the model cannot classify a message after three inbox checks, gosling
keeps it in the inbox, records that under **Mail activity**, and moves on. If SMTP
delivery is uncertain, check your mailbox before following up. gosling records the uncertainty
and leaves retry to you.
