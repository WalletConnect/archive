extern crate core;

use relay_rpc::{
    auth::{
        ed25519_dalek::Keypair,
        rand::{rngs::StdRng, SeedableRng},
        SerializedAuthToken,
    },
    domain::{ClientId, DecodedClientId},
};

mod context;
mod messages;
mod metrics;
mod registration;
mod simple;
mod storage;
mod webhooks;

const LOCALHOST_RELAY_URL: &str = "http://127.0.0.1:8080";

pub type ErrorResult<T> = Result<T, TestError>;

#[derive(Debug, thiserror::Error)]
pub enum TestError {
    #[error(transparent)]
    Elapsed(#[from] tokio::time::error::Elapsed),

    #[error(transparent)]
    Gilgamesh(#[from] gilgamesh::error::Error),
}

fn get_client_jwt(history_aud: String) -> (SerializedAuthToken, ClientId, Keypair) {
    let mut rng = StdRng::from_entropy();
    let keypair = Keypair::generate(&mut rng);

    let random_client_id = DecodedClientId(*keypair.public_key().as_bytes());
    let client_id = ClientId::from(random_client_id);

    let history_jwt = relay_rpc::auth::AuthToken::new(client_id.to_string())
        .aud(history_aud)
        .as_jwt(&keypair)
        .unwrap();

    (history_jwt, client_id, keypair)
}
