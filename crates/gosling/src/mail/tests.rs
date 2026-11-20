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
        recipients: vec!["owner@example.com".into()],
        folders: vec!["Receipts".into()],
        archive: String::new(),
        trash: String::new(),
        include_existing: false,
        local_archive: false,
    }
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
    });
    ledger.changed(&store).unwrap();
    assert!(ledger.check_revision(0).is_err());
    assert!(!ledger.snapshot().unwrap().to_string().contains(&secret_id));
    assert_eq!(store.read().unwrap().revision, 1);
}

#[tokio::test]
async fn one_time_code_authorizes_one_reply_and_stays_out_of_model_input() {
    let root = tempfile::tempdir().unwrap();
    let (store, mut ledger) = MailStore::open(root.path().to_path_buf()).unwrap();
    let policy = settings();
    ledger.account = Some(store::Account {
        settings: policy.clone(),
        secret_id: None,
        enabled: true,
        tested: true,
    });
    ledger.cursor = Some(Cursor::Imap {
        validity: 3,
        seen_through: 9,
    });
    let code = ledger
        .create_reply_code(&store, "owner@example.com")
        .unwrap();
    let secret = code.split_ascii_whitespace().last().unwrap();
    assert!(!ledger.snapshot().unwrap().to_string().contains(secret));
    assert!(!std::fs::read_to_string(root.path().join("mail.json"))
        .unwrap()
        .contains(secret));
    assert!(ledger
        .create_reply_code(&store, "outsider@example.net")
        .is_err());

    let raw = format!("From: owner@example.com\r\nSubject: Repeat {secret}\r\n\r\n{code}\r\nCan you help? {secret}");
    let mail = IncomingMessage::parse(raw.as_bytes()).unwrap();
    assert!(!mail.subject.contains(secret));
    assert!(!mail.body.contains(secret));
    assert!(mail.body.contains("Can you help?"));
    let reply = || Proposal::Reply {
        text: "Yes.".into(),
    };
    let authorization = record_action(&store, &mut ledger, &policy, Some((mail, reply())), None)
        .await
        .unwrap();
    assert!(authorization.is_some());

    drop(store);
    let (store, mut ledger) = MailStore::open(root.path().to_path_buf()).unwrap();
    let replay = IncomingMessage::parse(raw.as_bytes()).unwrap();
    assert!(
        record_action(&store, &mut ledger, &policy, Some((replay, reply())), None)
            .await
            .unwrap()
            .is_none()
    );
}
