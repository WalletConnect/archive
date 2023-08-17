use {
    dotenv::dotenv,
    gilgamesh::{config, error, log, state::AppState, store::mongo::MongoStore},
    std::sync::Arc,
    tokio::sync::broadcast,
};

#[tokio::main]
async fn main() -> error::Result<()> {
    let logger = log::Logger::init().expect("Failed to start logging");

    let (_signal, shutdown) = broadcast::channel(1);
    dotenv().ok();
    let config = config::get_config().expect(
        "Failed to load configuration, please ensure that all environment variables are defined.",
    );

    let store = Arc::new(MongoStore::new(&config.mongo_address).await?);

    let state = AppState::new(config.config, store.clone(), store.clone(), store.clone())?;

    let result = gilgamesh::bootstrap(shutdown, state).await;

    logger.stop();

    result
}
