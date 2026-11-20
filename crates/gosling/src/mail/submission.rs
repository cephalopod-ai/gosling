use super::{address, clean_text, store::Draft, Endpoint, Settings, MAX_REPLY, NETWORK_TIMEOUT};
use anyhow::{anyhow, bail, ensure, Result};
use lettre::message::{header, Mailbox};
use lettre::transport::smtp::{
    authentication::Credentials,
    client::{Tls, TlsParameters},
};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

pub(super) struct AuthorizedMessage(Message);

type Transport = AsyncSmtpTransport<Tokio1Executor>;

pub(super) fn transport(endpoint: &Endpoint, secret: &str) -> Result<Transport> {
    let parameters = TlsParameters::new(endpoint.host.clone())?;
    let tls = if endpoint.start_tls {
        Tls::Required(parameters)
    } else {
        Tls::Wrapper(parameters)
    };
    Ok(
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&endpoint.host)
            .tls(tls)
            .port(endpoint.port)
            .timeout(Some(NETWORK_TIMEOUT))
            .credentials(Credentials::new(endpoint.username.clone(), secret.into()))
            .build(),
    )
}

pub(super) async fn test(endpoint: &Endpoint, secret: &str) -> Result<()> {
    let server = format!("{}:{}", endpoint.host, endpoint.port);
    let transport = transport(endpoint, secret)
        .map_err(|_| anyhow!("Could not configure SMTP TLS for {server}"))?;
    let outcome = tokio::time::timeout(NETWORK_TIMEOUT, transport.test_connection()).await;
    match outcome {
        Ok(Ok(true)) => Ok(()),
        Err(_) => bail!("SMTP connection to {server} timed out"),
        Ok(Err(error)) if error.is_timeout() => bail!("SMTP connection to {server} timed out"),
        Ok(Err(error)) if error.is_tls() => bail!("SMTP TLS negotiation failed with {server}"),
        Ok(Err(error))
            if error
                .status()
                .is_some_and(|status| u16::from(status) == 535) =>
        {
            bail!("SMTP authentication was rejected by {server}")
        }
        _ => bail!("SMTP connection or authentication failed with {server}"),
    }
}

pub(super) fn authorized_message(
    settings: &Settings,
    draft: &Draft,
    original_subject: &str,
    id: &str,
) -> Result<AuthorizedMessage> {
    let recipient = address(&draft.recipient)?;
    ensure!(
        settings.recipients.contains(&recipient) && recipient != settings.address,
        "This recipient is no longer allowed"
    );
    ensure!(
        !draft.text.trim().is_empty()
            && draft.text.len() <= MAX_REPLY
            && draft
                .text
                .chars()
                .all(|ch| !ch.is_control() || matches!(ch, '\r' | '\n' | '\t')),
        "Invalid plain-text reply"
    );
    let sender = address(&settings.address)?;
    let (_, domain) = sender
        .rsplit_once('@')
        .ok_or_else(|| anyhow::anyhow!("Invalid dedicated mailbox"))?;
    let id: uuid::Uuid = id.parse()?;
    let subject = clean_text(original_subject, 160).replace(['\n', '\t', '\r'], " ");
    let subject = if subject.to_ascii_lowercase().starts_with("re:") {
        subject
    } else {
        format!("Re: {subject}")
    };
    let mut builder = Message::builder()
        .from(Mailbox::new(None, sender.parse()?))
        .to(Mailbox::new(None, recipient.parse()?))
        .subject(subject)
        .message_id(Some(format!("<{id}@{domain}>")))
        .raw_header(header::HeaderValue::new(
            header::HeaderName::new_from_ascii_str("Auto-Submitted"),
            "auto-replied".into(),
        ))
        .header(header::ContentType::TEXT_PLAIN);
    if let Some(thread) = draft
        .thread_id
        .as_deref()
        .filter(|value| super::valid_thread_id(value))
    {
        let id = format!("<{thread}>");
        builder = builder.in_reply_to(id.clone()).references(id);
    }
    let message = builder.body(draft.text.clone())?;
    let recipients = message.envelope().to();
    ensure!(
        recipients.len() == 1 && recipients[0].to_string() == recipient,
        "SMTP envelope failed recipient authorization"
    );
    Ok(AuthorizedMessage(message))
}

pub(super) async fn send(transport: Transport, message: AuthorizedMessage) -> Result<()> {
    transport.send(message.0).await?;
    Ok(())
}
