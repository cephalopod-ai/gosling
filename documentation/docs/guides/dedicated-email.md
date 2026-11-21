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

## Save notes and task requests

Turn on **Save notes and task requests locally** under **Advanced** to sort email text from
allowed senders into configured folders in **Mail activity**. An explicit filing request can
name a configured folder. If the model cannot determine a destination, the message goes to
**Unsorted**. The saved note contains bounded email text. Gosling keeps paper PDFs separately.

When the model identifies a question or instruction, Gosling saves it under **Task requests**.
Use **Open as chat draft** to review the text and submit it in a new Desktop chat. This mode
uses local capture in place of the mailbox's automatic filing and reply actions.

## Run signed tasks and import papers

Capture mode also recognizes signed questions and instructions, PDF attachments up to 8 MiB,
DOIs, and modern arXiv IDs or links. Completed signed requests show their responses in
**Mail activity**. Gosling opens a session and stores the answer. The built-in runner can
answer or transform text. For tool-backed work, set a local `task_runners` entry for that enrolled sender, using
the same absolute executable and arguments format as the research adapters. The runner
receives JSON with `version`, `sender`, `idempotencyKey`, `prompt`, `provider`, and `model`
on stdin and returns `{"answer":"…"}`. The configured runner controls the tools and workspace
that sender can use. The supplied `scripts/run-email-task.py` uses
`--gosling <binary> --workspace <directory>` and repeatable `--builtin <name>` arguments.
It uses a tool-free session by default. Explicit built-ins run through Gosling's ordinary
tool policies in Auto mode, with other profile extensions disabled. Unsigned requests remain
editable drafts.

Add an `email-workflows.json` file to the Gosling config directory. Its `task_signers` map
binds each enrolled email address to a Gosling secret holding a base64-encoded 32-byte key.
Install the same key on the sender's system as an owner-readable file. Use
`scripts/sign-email-task.py --from <sender> --to <mailbox> --subject <subject> --key-file <path>`
with the request body on stdin. It emits an RFC 822 message for your mail client or submission
tool. Send it preserving the custom headers and body. The sender also needs to be on the
Receive allowlist. A signature expires after 24 hours and can run only once.

In the same JSON file, the `research` map accepts `zotero`, `math`, and `physics` entries.
Each has an absolute `executable` and an `args` array. Use a Python interpreter as the
executable and the absolute path to `scripts/email-research.py` as its first argument. The
rest of the arguments identify the destination and its write scope:

| Destination | Adapter arguments after the script path |
|---|---|
| Zotero | `zotero --library users/<id> --collection <key> --key-file <path>`. Use a library-scoped write key in an owner-readable file. |
| Math MCP | `math --mathmcp <binary> --store <database> --capture-store <directory> --project <name> --license-note <note>`. Use a Python environment with Sampo and its PDF extra installed. |
| Alfred Physics | `physics --alfred <binary> --database <database>`. Use the intended Alfred research store. |

For example, a signed sender and one research destination can be configured as follows.

```json
{
  "task_signers": {
    "you@example.com": "GOSLING_MAIL_SIGNER_KEY"
  },
  "research": {
    "zotero": {
      "executable": "/absolute/path/to/python3",
      "args": [
        "/absolute/path/to/scripts/email-research.py",
        "zotero",
        "--library", "users/123456",
        "--collection", "ABCDEFGH",
        "--key-file", "/absolute/path/to/zotero-key"
      ]
    }
  }
}
```

These destinations are private write scopes chosen locally. Connector paths and arguments
come from this file. Mail text supplies article data and cannot modify the connector command.
The adapter receives a versioned JSON request on stdin and reports a destination receipt
as `{"id":"…"}`. Create the config file and key files with restricted permissions and use
disposable databases and a test Zotero collection for the first import.

For PDFs, Gosling retains the original locally and summarizes the extracted pages.
**Discard text and summary** in Mail activity clears the email text and generated summary.
The PDF stays in local research storage so research records can still refer to it.

For DOI messages without full text, Gosling can summarize a Crossref abstract when available. It fetches
arXiv PDFs from arXiv.org. Use **Summarize only**, **Zotero only**, **Math MCP only**, or
**Alfred only** as the subject or first line to narrow research intake. Research intake
shows unavailable sources, unavailable abstracts, and each destination's status. Unknown write outcomes require
checking the destination before using **Retry unverified imports**. The worker keeps the receipts
it already received and retries the other writes on a later inbox check.

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
activity, drafts and saved email text stay on this computer. Use **Mail activity** to
discard a draft or saved text. If the model cannot classify a message after three inbox checks, gosling
keeps it in the inbox, records that under **Mail activity**, and moves on. If SMTP
delivery is uncertain, check your mailbox before following up. gosling records the uncertainty
and leaves retry to you.
