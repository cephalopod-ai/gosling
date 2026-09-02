//! ACP byte-stream, HTTP connection, EOF, and stdio transport wiring.
//!
//! Maintainers: preserve protocol negotiation and connection shutdown behavior here.
//! Clients: public transport types and functions retain their compatibility facade paths.

use super::*;

pub struct GoslingAcpHandler {
    pub agent: Arc<GoslingAcpAgent>,
}

pub(super) fn negotiate_protocol_version(
    requested: ProtocolVersion,
) -> Result<ProtocolVersion, agent_client_protocol::Error> {
    if requested != ProtocolVersion::LATEST {
        return Err(agent_client_protocol::Error::invalid_params().data(format!(
            "Unsupported ACP protocol version {requested}; expected {}",
            ProtocolVersion::LATEST
        )));
    }
    Ok(ProtocolVersion::LATEST)
}

pub(super) struct EofAwareReader<R> {
    inner: R,
    eof_sender: Option<oneshot::Sender<()>>,
}

impl<R> EofAwareReader<R> {
    pub(super) fn new(inner: R, eof_sender: oneshot::Sender<()>) -> Self {
        Self {
            inner,
            eof_sender: Some(eof_sender),
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for EofAwareReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_read(cx, buffer);
        if matches!(result, Poll::Ready(Ok(0))) {
            if let Some(sender) = self.eof_sender.take() {
                let _ = sender.send(());
            }
        }
        result
    }
}

pub(super) async fn finish_connection_on_eof<F>(
    connection: F,
    eof_receiver: oneshot::Receiver<()>,
) -> Result<()>
where
    F: std::future::Future<Output = Result<(), agent_client_protocol::Error>>,
{
    match select(Box::pin(connection), Box::pin(eof_receiver)).await {
        Either::Left((result, _)) => result?,
        Either::Right((Ok(()), connection)) => {
            if let Ok(result) =
                tokio::time::timeout(std::time::Duration::from_secs(1), connection).await
            {
                result?;
            }
        }
        Either::Right((Err(_), connection)) => connection.await?,
    }
    Ok(())
}

pub fn serve<R, W>(
    agent: Arc<GoslingAcpAgent>,
    read: R,
    write: W,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
where
    R: futures::AsyncRead + Unpin + Send + 'static,
    W: futures::AsyncWrite + Unpin + Send + 'static,
{
    let runtime_paths = agent.runtime_paths.clone();
    Box::pin(Paths::scope(runtime_paths, async move {
        let handler = GoslingAcpHandler { agent };
        let (eof_sender, eof_receiver) = oneshot::channel();
        let read = EofAwareReader::new(read, eof_sender);

        let connection = SacpAgent
            .builder()
            .name("gosling-acp")
            .with_handler(handler)
            .connect_to(ByteStreams::new(write, read));

        finish_connection_on_eof(connection, eof_receiver).await
    }))
}

/// A lazily-initialized agent connection used by the HTTP/WebSocket transport.
///
/// The `agent-client-protocol-http` server takes a synchronous factory that
/// yields a [`ConnectTo<Client>`] per connection, but creating a gosling agent is
/// async. Agent creation is therefore deferred into [`ConnectTo::connect_to`],
/// which runs as the connection's serving future.
pub struct GoslingAgentConnection {
    server: Arc<crate::acp::server_factory::AcpServer>,
}

impl GoslingAgentConnection {
    pub fn new(server: Arc<crate::acp::server_factory::AcpServer>) -> Self {
        Self { server }
    }
}

impl agent_client_protocol::ConnectTo<Client> for GoslingAgentConnection {
    async fn connect_to(
        self,
        client: impl agent_client_protocol::ConnectTo<SacpAgent>,
    ) -> std::result::Result<(), agent_client_protocol::Error> {
        let agent = self.server.create_agent().await.internal_err()?;
        let handler = GoslingAcpHandler { agent };
        SacpAgent
            .builder()
            .name("gosling-acp")
            .with_handler(handler)
            .connect_to(client)
            .await
    }
}

pub async fn run(builtins: Vec<String>) -> Result<()> {
    info!("listening on stdio");

    let outgoing = tokio::io::stdout().compat_write();
    let incoming = tokio::io::stdin().compat();

    let server = crate::acp::server_factory::AcpServer::new(
        crate::acp::server_factory::AcpServerFactoryConfig {
            builtins,
            state_dir: Paths::state_dir(),
            data_dir: Paths::data_dir(),
            platform_data_dir: Paths::data_dir(),
            config_dir: Paths::config_dir(),
            gosling_platform: GoslingPlatform::GoslingCli,
            additional_source_roots: Vec::new(),
            shell_runtime: Default::default(),
        },
    );
    let agent = server.create_agent().await?;
    serve(agent, incoming, outgoing).await
}
