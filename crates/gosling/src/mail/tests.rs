use super::*;
use lettre::{AsyncSmtpTransport, Tokio1Executor};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

fn settings() -> Settings {
    let endpoint = Endpoint {
        host: "localhost".into(),
        port: 993,
        start_tls: false,
        username: "gosling@example.com".into(),
    };
    Settings {
        address: "gosling@example.com".into(),
        protocol: Protocol::Imap,
        incoming: endpoint.clone(),
        outgoing: endpoint,
        provider: "ollama".into(),
        model: "local".into(),
        instructions: String::new(),
        senders: vec!["owner@example.com".into(), "sender@example.org".into()],
        recipients: vec!["owner@example.com".into()],
        folders: vec!["Receipts".into()],
        archive: String::new(),
        trash: String::new(),
        include_existing: false,
        local_archive: false,
    }
}

#[test]
fn mail_instructions_default_to_empty_and_are_validated_when_saved() {
    let mut old_settings = serde_json::to_value(settings()).unwrap();
    old_settings.as_object_mut().unwrap().remove("instructions");
    let mut settings: Settings = serde_json::from_value(old_settings).unwrap();
    assert!(settings.instructions.is_empty());
    assert_eq!(classification_prompt("\n\t "), classification_prompt(""));

    settings.instructions = "Move receipts to Receipts.\nKeep replies brief.".into();
    settings.validate().unwrap();
    let restored: Settings =
        serde_json::from_value(serde_json::to_value(&settings).unwrap()).unwrap();
    assert_eq!(restored.instructions, settings.instructions);
    let prompt = classification_prompt(&restored.instructions);
    assert!(prompt.starts_with(&classification_prompt("")));
    assert!(prompt.ends_with(&settings.instructions));

    settings.instructions = "é".repeat(MAX_INSTRUCTIONS / 2);
    settings.validate().unwrap();
    settings.instructions.push('é');
    assert!(settings.validate().is_err());
    settings.instructions = "Instruction\0text".into();
    assert!(settings.validate().is_err());
}

fn draft_item(recipient: &str) -> Activity {
    let mut item = Activity::new("test subject".into(), "Reply ready for review");
    item.draft = Some(store::Draft {
        recipient: recipient.into(),
        text: "A plain-text answer.".into(),
        thread_id: Some("mail@example.com".into()),
    });
    item
}

#[test]
fn inbound_headers_cannot_override_the_configured_recipient() {
    let raw = b"From: Owner <owner@EXAMPLE.COM>\r\nReply-To: outsider@example.net\r\n\
        Cc: outsider@example.net\r\nMessage-ID: <mail@example.com>\r\n\
        Subject: Question\r\n\r\nWhat is in the mailbox?";
    let mail = IncomingMessage::parse(raw).unwrap();
    assert_eq!(
        reply_recipient(&settings(), &mail),
        Some("owner@example.com")
    );
    assert_eq!(mail.thread_id.as_deref(), Some("mail@example.com"));
    assert!(mail.body.contains("What is in the mailbox?"));
    let spoof_attempt = b"From: outsider@example.net\r\nReply-To: owner@example.com\r\n\r\nhello";
    let mail = IncomingMessage::parse(spoof_attempt).unwrap();
    assert_eq!(reply_recipient(&settings(), &mail), None);
    let duplicate_from = b"From: outsider@example.net\r\nFrom: owner@example.com\r\n\r\nhello";
    let mail = IncomingMessage::parse(duplicate_from).unwrap();
    assert_eq!(reply_recipient(&settings(), &mail), None);
    for header in [
        "Auto-Submitted: auto-replied",
        "Return-Path: <>",
        "List-Id: List <list.example.com>",
    ] {
        let raw = format!("From: owner@example.com\r\n{header}\r\n\r\nhello");
        let mail = IncomingMessage::parse(raw.as_bytes()).unwrap();
        assert_eq!(reply_recipient(&settings(), &mail), None, "{header}");
    }
}

#[tokio::test]
async fn revocation_and_uncertain_delivery_survive_restart() {
    let root = tempfile::tempdir().unwrap();
    let (store, mut ledger) = MailStore::open(root.path().to_path_buf()).unwrap();
    let item = draft_item("owner@example.com");
    let id = item.id.clone();
    ledger.add(item);
    store.write(&ledger).unwrap();
    assert!(MailStore::open(root.path().to_path_buf()).is_err());

    let mut policy = settings();
    policy.recipients.clear();
    assert!(submit_reply(&store, &mut ledger, &policy, &id, |_| async {
        panic!("revoked policy attempted SMTP")
    })
    .await
    .is_err());
    assert!(ledger.activity[0].draft.is_some());

    submit_reply(&store, &mut ledger, &settings(), &id, |_| async {
        let durable = store.read().unwrap();
        assert!(durable.activity[0].draft.is_none());
        assert!(durable.activity[0].status.contains("uncertain"));
        bail!("connection dropped after DATA")
    })
    .await
    .unwrap();
    drop(store);

    let (store, mut ledger) = MailStore::open(root.path().to_path_buf()).unwrap();
    assert!(ledger.activity[0].status.contains("uncertain"));
    assert!(
        submit_reply(&store, &mut ledger, &settings(), &id, |_| async {
            panic!("uncertain SMTP delivery was retried")
        })
        .await
        .is_err()
    );
}

#[tokio::test]
async fn smtp_envelope_and_headers_use_only_the_authorized_address() {
    let policy = settings();
    for recipient in [
        "outsider@example.net",
        "owner@example.com, outsider@example.net",
        "owner@example.com\r\nBcc: outsider@example.net",
    ] {
        let item = draft_item(recipient);
        assert!(submission::authorized_message(
            &policy,
            item.draft.as_ref().unwrap(),
            &item.subject,
            &item.id
        )
        .is_err());
    }

    let server = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = server.local_addr().unwrap().port();
    let transcript = tokio::spawn(async move {
        let (socket, _) = server.accept().await.unwrap();
        let mut client = BufReader::new(socket);
        client
            .get_mut()
            .write_all(b"220 localhost SMTP\r\n")
            .await
            .unwrap();
        let mut transcript = String::new();
        let mut data = false;
        loop {
            let mut line = String::new();
            if client.read_line(&mut line).await.unwrap() == 0 {
                break;
            }
            transcript.push_str(&line);
            let response: &[u8] = if data && line != ".\r\n" {
                continue;
            } else if line.starts_with("EHLO ") {
                b"250 localhost\r\n"
            } else if line == "DATA\r\n" {
                data = true;
                b"354 Continue\r\n"
            } else if line == "QUIT\r\n" {
                client
                    .get_mut()
                    .write_all(b"221 Goodbye\r\n")
                    .await
                    .unwrap();
                break;
            } else {
                data = false;
                b"250 Accepted\r\n"
            };
            client.get_mut().write_all(response).await.unwrap();
        }
        transcript
    });
    let item = draft_item("owner@example.com");
    let message = submission::authorized_message(
        &policy,
        item.draft.as_ref().unwrap(),
        &item.subject,
        &item.id,
    )
    .unwrap();
    let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
        .port(port)
        .build();
    tokio::time::timeout(Duration::from_secs(5), submission::send(transport, message))
        .await
        .unwrap()
        .unwrap();
    let transcript = tokio::time::timeout(Duration::from_secs(5), transcript)
        .await
        .unwrap()
        .unwrap();
    assert!(transcript.contains("MAIL FROM:<gosling@example.com>"));
    assert_eq!(transcript.matches("RCPT TO:").count(), 1);
    assert!(transcript.contains("RCPT TO:<owner@example.com>"));
    assert!(transcript.contains("To: owner@example.com"));
    assert!(transcript.contains("In-Reply-To: <mail@example.com>"));
    assert!(!transcript.contains("Bcc:"));
}

#[tokio::test]
async fn pop_local_trash_can_be_restored_without_a_server_operation() {
    let root = tempfile::tempdir().unwrap();
    let (store, mut ledger) = MailStore::open(root.path().to_path_buf()).unwrap();
    let mut policy = settings();
    policy.protocol = Protocol::Pop;
    ledger.account = Some(store::Account {
        settings: policy.clone(),
        secret_id: None,
        enabled: false,
        tested: false,
        smtp_tested: Some(false),
    });
    let message =
        IncomingMessage::parse(b"From: sender@example.org\r\nSubject: Junk\r\n\r\nBody").unwrap();
    record_action(
        &store,
        &mut ledger,
        &policy,
        Some((message, Proposal::Trash)),
        None,
    )
    .await
    .unwrap();
    let item = &ledger.activity[0];
    let id = item.id.clone();
    assert_eq!(item.local_folder.as_deref(), Some("Trash"));
    restore(&store, &mut ledger, &id).await.unwrap();
    assert_eq!(
        store.read().unwrap().activity[0].local_folder.as_deref(),
        Some("Inbox")
    );
}

#[tokio::test]
async fn repeated_classification_failure_releases_the_message() {
    let root = tempfile::tempdir().unwrap();
    let (store, mut ledger) = MailStore::open(root.path().to_path_buf()).unwrap();
    assert!(ledger.retry_classification("imap:7:41"));
    store.write(&ledger).unwrap();
    drop(store);

    let (store, mut ledger) = MailStore::open(root.path().to_path_buf()).unwrap();
    assert!(ledger.retry_classification("imap:7:41"));
    assert!(!ledger.retry_classification("imap:7:41"));
    assert!(ledger.retry_classification("imap:7:42"));
    ledger.classification_succeeded();
    assert!(ledger.retry_classification("imap:7:42"));
    assert!(ledger.retry_classification("imap:7:42"));
    assert!(!ledger.retry_classification("imap:7:42"));

    let message =
        IncomingMessage::parse(b"From: sender@example.org\r\nSubject: Hard\r\n\r\nBody").unwrap();
    record_action(
        &store,
        &mut ledger,
        &settings(),
        Some((message, Proposal::Unclassified)),
        None,
    )
    .await
    .unwrap();
    let item = &store.read().unwrap().activity[0];
    assert_eq!(item.subject, "Hard");
    assert!(item
        .status
        .starts_with("Kept in inbox. The mail model could not"));
    assert!(item.local_folder.is_none() && item.draft.is_none());
    assert!(serde_json::from_str::<Proposal>(r#"{"action":"unclassified"}"#).is_err());
}

#[test]
fn stale_policy_revision_and_secret_references_never_leave_storage() {
    let root = tempfile::tempdir().unwrap();
    let (store, mut ledger) = MailStore::open(root.path().to_path_buf()).unwrap();
    let secret_id = uuid::Uuid::new_v4().to_string();
    ledger.account = Some(store::Account {
        settings: settings(),
        secret_id: Some(secret_id.clone()),
        enabled: false,
        tested: false,
        smtp_tested: Some(false),
    });
    ledger.changed(&store).unwrap();
    assert!(ledger.check_revision(0).is_err());
    assert!(!ledger.snapshot().unwrap().to_string().contains(&secret_id));
    assert_eq!(store.read().unwrap().revision, 1);
}

#[tokio::test]
async fn a_reply_can_send_automatically_only_when_both_lists_allow_it() {
    let root = tempfile::tempdir().unwrap();
    let (store, mut ledger) = MailStore::open(root.path().to_path_buf()).unwrap();
    let mut policy = settings();
    policy.recipients.push("send-only@example.net".into());
    policy.provider = "invalid-provider-for-filter-test".into();
    ledger.account = Some(store::Account {
        settings: policy.clone(),
        secret_id: None,
        enabled: true,
        tested: true,
        smtp_tested: Some(true),
    });
    let reply = || Proposal::Reply {
        text: "Yes.".into(),
    };

    let raw = b"From: send-only@example.net\r\nSubject: Send only\r\n\r\nCan you help?";
    let rejected = decide(&store, &mut ledger, &policy, "pop:send-only", Some(raw))
        .await
        .unwrap();
    assert!(matches!(rejected, Some((_, Proposal::Keep))));
    let mail = IncomingMessage::parse(raw).unwrap();
    assert!(record_action(
        &store,
        &mut ledger,
        &policy,
        Some((mail, Proposal::Trash)),
        None
    )
    .await
    .unwrap()
    .is_none());
    assert!(ledger.activity.is_empty());

    let mail = IncomingMessage::parse(b"From: sender@example.org\r\n\r\nReceive only").unwrap();
    assert_eq!(allowed_sender(&policy, &mail), Some("sender@example.org"));
    assert!(
        record_action(&store, &mut ledger, &policy, Some((mail, reply())), None)
            .await
            .unwrap()
            .is_none()
    );
    assert!(ledger.activity[0].draft.is_none());

    let mail = IncomingMessage::parse(b"From: owner@example.com\r\n\r\nCan you help?").unwrap();
    let id = record_action(&store, &mut ledger, &policy, Some((mail, reply())), None)
        .await
        .unwrap()
        .unwrap();
    let persisted = store.read().unwrap();
    assert_eq!(persisted.activity[0].id, id);
    assert_eq!(
        persisted.activity[0].draft.as_ref().unwrap().recipient,
        "owner@example.com"
    );
}

#[tokio::test]
async fn legacy_mailbox_keeps_send_allowlist_and_waits_for_allowed_senders() {
    let root = tempfile::tempdir().unwrap();
    let (store, mut ledger) = MailStore::open(root.path().to_path_buf()).unwrap();
    ledger.account = Some(store::Account {
        settings: settings(),
        secret_id: None,
        enabled: true,
        tested: true,
        smtp_tested: Some(true),
    });
    let mut old = serde_json::to_value(&ledger).unwrap();
    old["account"]["settings"]
        .as_object_mut()
        .unwrap()
        .remove("senders");
    old["reply_codes"] =
        serde_json::json!([{ "recipient": "owner@example.com", "digest": "legacy", "expires": 0 }]);
    let mut ledger: Ledger = serde_json::from_value(old).unwrap();
    assert!(ledger.account().unwrap().settings.senders.is_empty());
    assert_eq!(
        ledger.account().unwrap().settings.recipients,
        ["owner@example.com"]
    );
    poll(&store, &mut ledger).await.unwrap();
    assert!(ledger.cursor.is_none());
    assert!(serde_json::to_value(&ledger)
        .unwrap()
        .get("reply_codes")
        .is_none());
}

#[test]
fn previously_issued_codes_stay_out_of_model_input() {
    let secret = "0123456789abcdef0123456789abcdef";
    let raw = format!("From: owner@example.com\r\nSubject: Repeat {secret}\r\n\r\nGosling reply code: {secret}\r\nCan you help? {secret}");
    let mail = IncomingMessage::parse(raw.as_bytes()).unwrap();
    assert!(!mail.subject.contains(secret));
    assert!(!mail.body.contains(secret));
    assert!(mail.body.contains("Can you help?"));
}

#[tokio::test]
async fn failed_smtp_test_keeps_inbox_active_but_blocks_sending() {
    let root = tempfile::tempdir().unwrap();
    let (store, mut ledger) = MailStore::open(root.path().to_path_buf()).unwrap();
    let policy = settings();
    let mut legacy = serde_json::json!({
        "settings": policy,
        "secret_id": null,
        "enabled": true,
        "tested": true,
    });
    ledger.account = Some(serde_json::from_value(legacy.clone()).unwrap());
    legacy["tested"] = false.into();
    assert!(!serde_json::from_value::<store::Account>(legacy)
        .unwrap()
        .smtp_tested());
    ledger
        .record_connection_test(&store, Ok(()), Err(anyhow::anyhow!("SMTP timed out")))
        .unwrap();

    let mut ledger = store.read().unwrap();
    assert!(ledger.account().unwrap().enabled);
    assert!(ledger.account().unwrap().tested);
    assert!(!ledger.account().unwrap().smtp_tested());
    let snapshot = ledger.snapshot().unwrap();
    assert_eq!(snapshot["account"]["smtpTested"], false);
    assert_eq!(
        snapshot["lastStatus"],
        "Inbox connection verified. SMTP timed out."
    );
    let mail =
        IncomingMessage::parse(b"From: owner@example.com\r\nSubject: Test\r\n\r\nCan you help?")
            .unwrap();
    let draft = Proposal::Reply {
        text: "Yes.".into(),
    };
    assert!(
        record_action(&store, &mut ledger, &policy, Some((mail, draft)), None)
            .await
            .unwrap()
            .is_none()
    );
    let id = ledger.activity[0].id.clone();
    assert!(send_reply(&store, &mut ledger, &id)
        .await
        .unwrap_err()
        .to_string()
        .contains("Verify SMTP"));
    assert!(ledger.activity[0].draft.is_some());

    ledger
        .record_connection_test(&store, Ok(()), Ok(()))
        .unwrap();
    assert!(ledger.account().unwrap().can_send());
}
