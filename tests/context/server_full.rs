use {
    super::server::{Gilgamesh, Options},
    crate::LOCALHOST_RELAY_URL,
    async_trait::async_trait,
    gilgamesh::{config::DEFAULT_RELAY_URL, store::mongo::MongoStore},
    std::{env, sync::Arc},
    test_context::AsyncTestContext,
};

fn url(env: &str) -> String {
    let domain = "history.walletconnect.com";
    let domain = match env {
        "prod" => domain.to_owned(),
        "staging" => format!("staging.{domain}"),
        "dev" => format!("dev.{domain}"),
        _ => panic!("Unsupported environment {env}"),
    };
    format!("https://{domain}")
}

// Normal server with nothing mocked
pub struct ServerFullContext {
    pub server_url: String,
    pub server: Option<Gilgamesh>,
    pub relay_url: String,
}

#[async_trait]
impl AsyncTestContext for ServerFullContext {
    async fn setup() -> Self {
        if let Ok(env) = env::var("ENVIRONMENT") {
            let server_url = url(&env);
            Self {
                server_url,
                server: None,
                relay_url: DEFAULT_RELAY_URL.to_owned(),
            }
        } else {
            let relay_url = LOCALHOST_RELAY_URL.to_owned();
            let mongo_address = env::var("MONGO_ADDRESS").unwrap_or_else(|_| {
                "mongodb://admin:admin@localhost:27018/gilgamesh?authSource=admin".to_owned()
            });
            let store = Arc::new(MongoStore::new(&mongo_address).await.unwrap());
            let options = Options {
                relay_url: relay_url.clone(),
                message_store: store.clone(),
                registration_store: store.clone(),
                registration2_store: store.clone(),
            };
            let server = Gilgamesh::start(options).await;
            Self {
                relay_url,
                server_url: server.public_url.clone(),
                server: Some(server),
            }
        }
    }

    async fn teardown(mut self) {
        if let Some(mut server) = self.server {
            server.shutdown().await;
        }
    }
}
