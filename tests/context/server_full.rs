use {
    super::server::{Gilgamesh, Options},
    async_trait::async_trait,
    gilgamesh::store::mongo::MongoStore,
    std::{env, sync::Arc},
    test_context::AsyncTestContext,
};

fn url(env: &str) -> String {
    let domain = "history.walletconnect.com";
    match env {
        "prod" => domain.to_owned(),
        "staging" => format!("staging.{domain}"),
        "dev" => format!("dev.{domain}"),
        _ => panic!("Unsupported environment {env}"),
    }
}

// Normal server with nothing mocked
pub struct ServerFullContext {
    pub server_url: String,
    pub server: Option<Gilgamesh>,
}

#[async_trait]
impl AsyncTestContext for ServerFullContext {
    async fn setup() -> Self {
        if let Ok(env) = env::var("ENVIRONMENT") {
            let server_url = url(&env);
            Self {
                server_url,
                server: None,
            }
        } else {
            let mongo_address = env::var("MONGO_ADDRESS").unwrap_or_else(|_| {
                "mongodb://admin:admin@localhost:27018/gilgamesh?authSource=admin".to_owned()
            });
            let store = Arc::new(MongoStore::new(&mongo_address).await.unwrap());
            let options = Options {
                message_store: store.clone(),
                registration_store: store.clone(),
                registration2_store: store.clone(),
            };
            let server = Gilgamesh::start(options).await;
            Self {
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
