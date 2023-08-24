use {
    crate::LOCALHOST_RELAY_URL,
    async_trait::async_trait,
    gilgamesh::{
        config::Configuration,
        state::{AppState, MessageStorageArc, Registration2StorageArc, RegistrationStorageArc},
        store::mocks::{
            messages::MockMessageStore,
            registrations::MockRegistrationStore,
            registrations2::MockRegistration2Store,
        },
    },
    std::{
        net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener},
        sync::Arc,
    },
    test_context::AsyncTestContext,
    tokio::{
        runtime::Handle,
        sync::broadcast,
        time::{sleep, Duration},
    },
};

pub struct Options {
    pub relay_url: String,
    pub message_store: MessageStorageArc,
    pub registration_store: RegistrationStorageArc,
    pub registration2_store: Registration2StorageArc,
}

pub struct Gilgamesh {
    pub public_addr: SocketAddr,
    pub public_url: String,
    shutdown_signal: broadcast::Sender<()>,
    is_shutdown: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {}

impl Gilgamesh {
    pub async fn start(options: Options) -> Self {
        let public_port = get_random_port();
        let rt = Handle::current();
        let public_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), public_port);
        let public_url = format!("http://{public_addr}");

        let (signal, shutdown) = broadcast::channel(1);

        std::thread::spawn({
            let public_url = public_url.clone();
            move || {
                rt.block_on(async move {
                    let config = Configuration {
                        port: public_port,
                        public_url,
                        log_level: "info".to_owned(),
                        relay_url: options.relay_url,
                        validate_signatures: false,
                        is_test: true,
                        otel_exporter_otlp_endpoint: None,
                        telemetry_prometheus_port: Some(get_random_port()),
                    };

                    let state = AppState::new(
                        config,
                        options.message_store,
                        options.registration_store,
                        options.registration2_store,
                    )?;

                    gilgamesh::bootstrap(shutdown, state).await
                })
                .unwrap();
            }
        });

        if let Err(e) = wait_for_server_to_start(public_port).await {
            panic!("Failed to start server with error: {e:?}")
        }

        Self {
            public_addr,
            public_url,
            shutdown_signal: signal,
            is_shutdown: false,
        }
    }

    pub async fn shutdown(&mut self) {
        if self.is_shutdown {
            return;
        }
        self.is_shutdown = true;
        let _ = self.shutdown_signal.send(());
        wait_for_server_to_shutdown(self.public_addr.port())
            .await
            .unwrap();
    }
}

// Finds a free port.
pub fn get_random_port() -> u16 {
    use std::sync::atomic::{AtomicU16, Ordering};

    static NEXT_PORT: AtomicU16 = AtomicU16::new(9000);

    loop {
        let port = NEXT_PORT.fetch_add(1, Ordering::SeqCst);

        if is_port_available(port) {
            return port;
        }
    }
}

fn is_port_available(port: u16) -> bool {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)).is_ok()
}

async fn wait_for_server_to_shutdown(port: u16) -> crate::ErrorResult<()> {
    let poll_fut = async {
        while !is_port_available(port) {
            sleep(Duration::from_millis(10)).await;
        }
    };

    Ok(tokio::time::timeout(Duration::from_secs(3), poll_fut).await?)
}

async fn wait_for_server_to_start(port: u16) -> crate::ErrorResult<()> {
    let poll_fut = async {
        while is_port_available(port) {
            sleep(Duration::from_millis(10)).await;
        }
    };

    Ok(tokio::time::timeout(Duration::from_secs(5), poll_fut).await?)
}

// Server with mocked stoage
pub struct ServerContext {
    pub relay_url: String,
    pub server: Gilgamesh,
    pub message_store: Arc<MockMessageStore>,
    pub registration_store: Arc<MockRegistrationStore>,
    pub registration2_store: Arc<MockRegistration2Store>,
}

#[async_trait]
impl AsyncTestContext for ServerContext {
    async fn setup() -> Self {
        let relay_url = LOCALHOST_RELAY_URL.to_owned();
        let message_store = Arc::new(MockMessageStore::new());
        let registration_store = Arc::new(MockRegistrationStore::new());
        let registration2_store = Arc::new(MockRegistration2Store::new());
        let server = Gilgamesh::start(Options {
            relay_url: relay_url.clone(),
            message_store: message_store.clone(),
            registration_store: registration_store.clone(),
            registration2_store: registration2_store.clone(),
        })
        .await;
        Self {
            relay_url,
            server,
            message_store,
            registration_store,
            registration2_store,
        }
    }

    async fn teardown(mut self) {
        self.server.shutdown().await;
    }
}
