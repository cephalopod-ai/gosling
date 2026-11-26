use super::{store::Trashed, Endpoint, Protocol, Settings};
use anyhow::{bail, ensure, Context, Result};
use async_imap::imap_proto::{Response, ResponseCode, Status, UidSetMember};
use async_imap::types::NameAttribute;
use futures::TryStreamExt;
use std::{collections::BTreeSet, fmt::Debug, sync::Arc};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::rustls::{self, pki_types::ServerName};

pub(super) const INBOX: &str = "INBOX";

pub(super) trait Stream: AsyncRead + AsyncWrite + Unpin + Send + Debug {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + Debug> Stream for T {}
type Wire = Box<dyn Stream>;
pub(super) type Imap = async_imap::Session<Wire>;

pub(super) async fn imap(endpoint: &Endpoint, password: &str) -> Result<Imap> {
    let socket = TcpStream::connect((endpoint.host.as_str(), endpoint.port)).await?;
    let stream = if endpoint.start_tls {
        let mut plain = BufReader::new(socket);
        let greeting = read_line(&mut plain, 8192).await?;
        ensure!(
            greeting == b"* OK\r\n" || greeting.starts_with(b"* OK "),
            "IMAP greeting failed"
        );
        plain.get_mut().write_all(b"g1 STARTTLS\r\n").await?;
        plain.get_mut().flush().await?;
        let response = read_line(&mut plain, 8192).await?;
        ensure!(
            (response == b"g1 OK\r\n" || response.starts_with(b"g1 OK "))
                && plain.buffer().is_empty(),
            "IMAP STARTTLS failed"
        );
        secure_stream(plain.into_inner(), &endpoint.host).await?
    } else {
        secure_stream(socket, &endpoint.host).await?
    };
    let mut client = async_imap::Client::new(stream);
    if !endpoint.start_tls {
        let greeting = client
            .read_response()
            .await?
            .context("Missing IMAP greeting")?;
        ensure!(
            matches!(
                greeting.parsed(),
                Response::Data {
                    status: Status::Ok,
                    ..
                }
            ),
            "IMAP greeting failed"
        );
    }
    client
        .login(&endpoint.username, password)
        .await
        .map_err(|_| anyhow::anyhow!("IMAP login failed"))
}

async fn secure_stream<T: Stream + 'static>(socket: T, server: &str) -> Result<Wire> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
    ensure!(
        !roots.is_empty(),
        "No system TLS certificates are available"
    );
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_root_certificates(roots)
    .with_no_client_auth();
    let server = ServerName::try_from(server.to_owned())?;
    Ok(Box::new(
        tokio_rustls::TlsConnector::from(Arc::new(config))
            .connect(server, socket)
            .await?,
    ))
}

pub(super) async fn test(settings: &Settings, password: &str) -> Result<()> {
    match settings.protocol {
        Protocol::Imap => {
            let mut session = imap(&settings.incoming, password).await?;
            let mailbox = session.examine(INBOX).await?;
            ensure!(
                mailbox.uid_validity.is_some() && mailbox.uid_next.is_some(),
                "IMAP UID metadata unavailable"
            );
        }
        Protocol::Pop => {
            let mut session = pop(&settings.incoming, password).await?;
            session.ids().await?;
        }
    }
    Ok(())
}

pub(super) async fn fetch_imap(
    session: &mut Imap,
    uid: u32,
    message_limit: usize,
) -> Result<Option<Vec<u8>>> {
    let query = format!("(UID RFC822.SIZE BODY.PEEK[]<0.{}>)", message_limit + 1);
    let mut rows = session.uid_fetch(uid.to_string(), &query).await?;
    let mut message = None;
    while let Some(row) = rows.try_next().await? {
        if row.uid == Some(uid) && row.size.is_some_and(|size| size as usize <= message_limit) {
            if let Some(body) = row.body().filter(|body| body.len() <= message_limit) {
                message = Some(body.to_vec());
            }
        }
    }
    Ok(message)
}

pub(super) async fn special_folder(session: &mut Imap, trash: bool) -> Result<Option<String>> {
    let mut names = session.list(Some(""), Some("*")).await?;
    let mut all = None;
    while let Some(name) = names.try_next().await? {
        let preferred = if trash {
            NameAttribute::Trash
        } else {
            NameAttribute::Archive
        };
        if name.attributes().contains(&preferred) && !name.name().eq_ignore_ascii_case(INBOX) {
            let found = name.name().to_owned();
            while names.try_next().await?.is_some() {}
            return Ok(Some(found));
        }
        if !trash
            && name.attributes().contains(&NameAttribute::All)
            && !name.name().eq_ignore_ascii_case(INBOX)
        {
            all = Some(name.name().to_owned());
        }
    }
    Ok(all)
}

pub(super) async fn can_move(session: &mut Imap) -> Result<bool> {
    Ok(session.capabilities().await?.has_str("MOVE"))
}

pub(super) async fn move_uid(
    session: &mut Imap,
    uid: u32,
    destination: &str,
) -> Result<Option<Trashed>> {
    ensure!(
        super::valid_line(destination, 512),
        "Invalid IMAP folder name"
    );
    let quoted = destination.replace('\\', "\\\\").replace('"', "\\\"");
    let request = session
        .run_command(format!("UID MOVE {uid} \"{quoted}\""))
        .await?;
    let mut moved = None;
    while let Some(response) = session.read_response().await? {
        let (status, code, done) = match response.parsed() {
            Response::Done {
                tag, status, code, ..
            } if tag == &request => (status, code, true),
            Response::Data { status, code, .. } => (status, code, false),
            _ => continue,
        };
        if *status == Status::Ok {
            if let Some(ResponseCode::CopyUid(validity, source, target)) = code {
                if single_uid(source) == Some(uid) {
                    moved = single_uid(target).map(|uid| Trashed {
                        folder: destination.to_owned(),
                        uid,
                        validity: *validity,
                    });
                }
            }
        }
        if done {
            ensure!(*status == Status::Ok, "Server refused IMAP MOVE");
            return Ok(moved);
        }
    }
    bail!("IMAP MOVE response was lost");
}

fn single_uid(set: &[UidSetMember]) -> Option<u32> {
    match set {
        [UidSetMember::Uid(uid)] => Some(*uid),
        [UidSetMember::UidRange(range)] if range.start() == range.end() => Some(*range.start()),
        _ => None,
    }
}

pub(super) struct Pop(BufReader<Wire>);

pub(super) async fn pop(endpoint: &Endpoint, password: &str) -> Result<Pop> {
    let socket = TcpStream::connect((endpoint.host.as_str(), endpoint.port)).await?;
    let mut session = if endpoint.start_tls {
        let mut plain = Pop(BufReader::new(Box::new(socket)));
        plain.positive().await?;
        plain.command("STLS").await?;
        ensure!(
            plain.0.buffer().is_empty(),
            "Unexpected data before POP TLS upgrade"
        );
        Pop(BufReader::new(
            secure_stream(plain.0.into_inner(), &endpoint.host).await?,
        ))
    } else {
        let mut secure = Pop(BufReader::new(secure_stream(socket, &endpoint.host).await?));
        secure.positive().await?;
        secure
    };
    session
        .command(&format!("USER {}", endpoint.username))
        .await?;
    session.command(&format!("PASS {password}")).await?;
    Ok(session)
}

impl Pop {
    async fn positive(&mut self) -> Result<Vec<u8>> {
        let line = read_line(&mut self.0, 8192).await?;
        ensure!(
            line == b"+OK\r\n" || line.starts_with(b"+OK "),
            "POP server rejected a command"
        );
        Ok(line)
    }

    async fn command(&mut self, command: &str) -> Result<Vec<u8>> {
        self.0.get_mut().write_all(command.as_bytes()).await?;
        self.0.get_mut().write_all(b"\r\n").await?;
        self.0.get_mut().flush().await?;
        self.positive().await
    }

    pub async fn ids(&mut self) -> Result<Vec<(u32, String)>> {
        self.command("UIDL").await?;
        let mut ids = Vec::new();
        let mut unique = BTreeSet::new();
        let mut numbers = BTreeSet::new();
        loop {
            let line = read_line(&mut self.0, 256).await?;
            if line == b".\r\n" {
                return Ok(ids);
            }
            let raw = std::str::from_utf8(&line)?;
            let mut columns = raw.split_ascii_whitespace();
            let number: u32 = columns
                .next()
                .context("Missing POP message number")?
                .parse()?;
            let id = columns.next().context("Missing POP UIDL")?;
            ensure!(
                columns.next().is_none()
                    && number > 0
                    && id.len() <= 70
                    && id.bytes().all(|byte| byte.is_ascii_graphic())
                    && unique.insert(id.to_owned())
                    && numbers.insert(number)
                    && ids.len() < 50_000,
                "Invalid or oversized POP UIDL listing"
            );
            ids.push((number, id.to_owned()));
        }
    }

    pub async fn fetch(&mut self, number: u32, message_limit: usize) -> Result<Option<Vec<u8>>> {
        let listing = self.command(&format!("LIST {number}")).await?;
        let mut columns = std::str::from_utf8(&listing)?
            .split_ascii_whitespace()
            .skip(1);
        let listed_number = columns
            .next()
            .context("Missing POP message number")?
            .parse::<u32>()?;
        let size = columns
            .next()
            .context("Missing POP message size")?
            .parse::<usize>()?;
        ensure!(listed_number == number, "Unexpected POP message number");
        if size > message_limit {
            return Ok(None);
        }
        self.command(&format!("RETR {number}")).await?;
        let mut contents = Vec::new();
        loop {
            let Some(line) = read_bounded_line(&mut self.0, message_limit + 2).await? else {
                return Ok(None);
            };
            if line == b".\r\n" {
                return Ok(Some(contents));
            }
            let line = if line.starts_with(b"..") {
                &line[1..]
            } else {
                &line
            };
            if contents.len() + line.len() > message_limit {
                return Ok(None);
            }
            contents.extend_from_slice(line);
        }
    }
}

async fn read_line(stream: &mut (impl AsyncBufRead + Unpin), limit: usize) -> Result<Vec<u8>> {
    read_bounded_line(stream, limit)
        .await?
        .context("Mail server line is too large")
}

async fn read_bounded_line(
    stream: &mut (impl AsyncBufRead + Unpin),
    limit: usize,
) -> Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let buffer = stream.fill_buf().await?;
        ensure!(!buffer.is_empty(), "Mail server closed the connection");
        let end = buffer.iter().position(|byte| *byte == b'\n');
        let consumed = end.map_or(buffer.len(), |index| index + 1);
        if line.len() + consumed > limit {
            return Ok(None);
        }
        line.extend_from_slice(&buffer[..consumed]);
        stream.consume(consumed);
        if end.is_some() {
            ensure!(line.ends_with(b"\r\n"), "Invalid mail server line ending");
            return Ok(Some(line));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn imap_move_tracks_the_destination_uid() {
        let (client, server) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move {
            let mut server = BufReader::new(server);
            let mut line = String::new();
            server.read_line(&mut line).await.unwrap();
            let tag = line.split_ascii_whitespace().next().unwrap();
            let response = format!("{tag} OK logged in\r\n");
            server
                .get_mut()
                .write_all(response.as_bytes())
                .await
                .unwrap();
            line.clear();
            server.read_line(&mut line).await.unwrap();
            let (tag, command) = line.trim_end().split_once(' ').unwrap();
            assert_eq!(command, "UID MOVE 42 \"Mail/Trash\"");
            let response =
                format!("* OK [COPYUID 77 42 901] copied\r\n* 1 EXPUNGE\r\n{tag} OK moved\r\n");
            server
                .get_mut()
                .write_all(response.as_bytes())
                .await
                .unwrap();
        });
        let client = async_imap::Client::new(Box::new(client) as Wire);
        let mut session = client.login("owner", "test-password").await.unwrap();
        let record = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            move_uid(&mut session, 42, "Mail/Trash"),
        )
        .await
        .unwrap()
        .unwrap()
        .unwrap();
        assert_eq!((record.validity, record.uid), (77, 901));
        assert_eq!(record.folder, "Mail/Trash");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn pop_skips_a_message_when_the_actual_line_exceeds_list_size() {
        let (client, server) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move {
            let mut server = BufReader::new(server);
            let mut command = String::new();
            server.read_line(&mut command).await.unwrap();
            assert_eq!(command, "LIST 1\r\n");
            server.get_mut().write_all(b"+OK 1 4\r\n").await.unwrap();
            command.clear();
            server.read_line(&mut command).await.unwrap();
            assert_eq!(command, "RETR 1\r\n");
            server
                .get_mut()
                .write_all(b"+OK\r\n123456789\r\n.\r\n")
                .await
                .unwrap();
        });
        let mut session = Pop(BufReader::new(Box::new(client)));
        let result = tokio::time::timeout(std::time::Duration::from_secs(3), session.fetch(1, 4))
            .await
            .unwrap();
        assert!(result.unwrap().is_none());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn pop_does_not_authenticate_when_starttls_is_refused() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (peer, _) = listener.accept().await.unwrap();
            let mut peer = BufReader::new(peer);
            peer.get_mut().write_all(b"+OK ready\r\n").await.unwrap();
            let mut line = String::new();
            peer.read_line(&mut line).await.unwrap();
            assert_eq!(line, "STLS\r\n");
            peer.get_mut()
                .write_all(b"-ERR unavailable\r\n")
                .await
                .unwrap();
            line.clear();
            assert_eq!(peer.read_line(&mut line).await.unwrap(), 0);
        });
        let endpoint = Endpoint {
            host: "127.0.0.1".into(),
            port,
            start_tls: true,
            username: "owner".into(),
        };
        assert!(pop(&endpoint, "test-password").await.is_err());
        server.await.unwrap();
    }
}
