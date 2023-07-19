use {
    crate::{
        error,
        metrics::Metrics,
        relay::RelayClient,
        store::{
            messages::MessagesStore,
            registrations::RegistrationStore,
            registrations2::Registration2Store,
        },
        Configuration,
    },
    build_info::BuildInfo,
    moka::future::Cache,
    std::{collections::HashSet, sync::Arc, time::Duration},
};

pub type MessageStorageArc = Arc<dyn MessagesStore + Send + Sync + 'static>;
pub type RegistrationStorageArc = Arc<dyn RegistrationStore + Send + Sync + 'static>;
pub type Registration2StorageArc = Arc<dyn Registration2Store + Send + Sync + 'static>;

#[derive(Clone)]
pub struct CachedRegistration {
    pub tags: Vec<Arc<str>>,
    pub relay_url: Arc<str>,
}

#[derive(Clone)]
pub struct CachedRegistration2 {
    pub tags: Vec<u32>,
    pub relay_url: Arc<str>,
    pub relay_id: Arc<str>,
}

pub trait State {
    fn config(&self) -> Configuration;
    fn build_info(&self) -> BuildInfo;
    fn message_store(&self) -> MessageStorageArc;
    fn relay_client(&self) -> RelayClient;
    fn validate_signatures(&self) -> bool;
}

#[derive(Clone)]
pub struct AppState {
    pub config: Configuration,
    pub build_info: BuildInfo,
    pub metrics: Option<Metrics>,
    pub message_store: MessageStorageArc,
    pub registration_store: RegistrationStorageArc,
    pub registration_cache: Cache<Arc<str>, CachedRegistration>,
    pub registration2_store: Registration2StorageArc,
    pub registration2_cache: Cache<Arc<str>, CachedRegistration2>,
    pub relay_client: RelayClient,
    pub auth_aud: HashSet<String>,
}

build_info::build_info!(fn build_info);

impl AppState {
    pub fn new(
        config: Configuration,
        message_store: MessageStorageArc,
        registration_store: RegistrationStorageArc,
        registration2_store: Registration2StorageArc,
    ) -> error::Result<AppState> {
        // Check config is valid and then throw the error if its not
        config.is_valid()?;

        let build_info: &BuildInfo = build_info();

        let relay_url = config.relay_url.to_string();

        let registration_cache = Cache::builder()
            .weigher(|_key, value: &CachedRegistration| -> u32 {
                let url_weight = value.relay_url.len();
                let tag_weight = value.tags.iter().map(|tag| tag.len()).sum::<usize>();
                (url_weight + tag_weight).try_into().unwrap_or(u32::MAX)
            })
            .max_capacity(32 * 1024 * 1024)
            .time_to_live(Duration::from_secs(30 * 60))
            .time_to_idle(Duration::from_secs(5 * 60))
            .build();

        let registration2_cache = Cache::builder()
            .max_capacity(32 * 1024 * 1024)
            .time_to_live(Duration::from_secs(30 * 60))
            .time_to_idle(Duration::from_secs(5 * 60))
            .build();

        let aud = config.public_url.clone();
        Ok(AppState {
            config,
            build_info: build_info.clone(),
            metrics: None,
            message_store,
            registration_store,
            registration_cache,
            registration2_store,
            registration2_cache,
            relay_client: RelayClient::new(relay_url),
            auth_aud: [aud].into(),
        })
    }

    pub fn set_metrics(&mut self, metrics: Metrics) {
        self.metrics = Some(metrics);
    }
}

impl State for Arc<AppState> {
    fn config(&self) -> Configuration {
        self.config.clone()
    }

    fn build_info(&self) -> BuildInfo {
        self.build_info.clone()
    }

    fn message_store(&self) -> MessageStorageArc {
        self.message_store.clone()
    }

    fn relay_client(&self) -> RelayClient {
        self.relay_client.clone()
    }

    fn validate_signatures(&self) -> bool {
        self.config.validate_signatures
    }
}
