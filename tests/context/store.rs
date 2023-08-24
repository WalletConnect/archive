use {
    super::server::{Gilgamesh, Options},
    crate::LOCALHOST_RELAY_URL,
    async_trait::async_trait,
    gilgamesh::store::mongo::MongoStore,
    std::{env, sync::Arc},
    test_context::AsyncTestContext,
};

#[derive(Clone)]
pub struct PersistentStorage {
    pub store: MongoStore,
}

impl PersistentStorage {
    pub async fn init() -> Self {
        let mongo_address = env::var("MONGO_ADDRESS").unwrap_or_else(|_| {
            "mongodb://admin:admin@localhost:27018/gilgamesh?authSource=admin".to_owned()
        });

        let storage = MongoStore::new(&mongo_address).await.unwrap();

        Self { store: storage }
    }

    pub async fn shutdown(&mut self) {}
}

#[derive(Clone)]
pub struct StoreContext {
    pub storage: PersistentStorage,
}

#[async_trait]
impl AsyncTestContext for StoreContext {
    async fn setup() -> Self {
        let storage = PersistentStorage::init().await;
        Self { storage }
    }

    async fn teardown(mut self) {
        self.storage.shutdown().await;
    }
}

pub struct ServerStoreContext {
    pub server: Gilgamesh,
    pub storage: PersistentStorage,
}

#[async_trait]
impl AsyncTestContext for ServerStoreContext {
    async fn setup() -> Self {
        let mongo_address = env::var("MONGO_ADDRESS")
            .unwrap_or("mongodb://admin:admin@mongo:27018/gilgamesh?authSource=admin".into());
        let store = Arc::new(MongoStore::new(&mongo_address).await.unwrap());
        let options = Options {
            relay_url: LOCALHOST_RELAY_URL.to_owned(),
            message_store: store.clone(),
            registration_store: store.clone(),
            registration2_store: store.clone(),
        };
        let server = Gilgamesh::start(options).await;
        let storage = PersistentStorage::init().await;
        Self { server, storage }
    }

    async fn teardown(mut self) {
        self.server.shutdown().await;
        self.storage.shutdown().await;
    }
}
