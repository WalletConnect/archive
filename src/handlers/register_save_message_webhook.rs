use {
    crate::{
        auth::AuthBearer,
        error,
        handlers::{Response, ResponseError},
        increment_counter,
        log::prelude::*,
        state::{AppState, CachedRegistration2},
    },
    axum::{extract::State, Json},
    hyper::StatusCode,
    relay_client::{http::Client, ConnectionOptions},
    relay_rpc::{
        auth::{
            ed25519_dalek::Keypair,
            rand::{rngs::StdRng, SeedableRng},
            AuthToken,
        },
        domain::ClientId,
        jwt::{JwtBasicClaims, VerifyableClaims},
        rpc::WatchRegisterClaims,
    },
    serde::{Deserialize, Serialize},
    std::sync::Arc,
};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegisterPayload {
    pub jwt: Arc<str>,
    pub tags: Vec<u32>,
    pub relay_url: Arc<str>,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    AuthBearer(token): AuthBearer,
    Json(body): Json<RegisterPayload>,
) -> error::Result<Response> {
    // claims & client ID for Bearer token
    let claims = JwtBasicClaims::try_from_str(&token)?;
    claims.verify_basic(&state.auth_aud, None)?;

    // claims & client ID for pass-through watchRegister JWT
    let watch_register_claims = WatchRegisterClaims::try_from_str(&body.jwt)?;
    if watch_register_claims.basic.iss != claims.iss {
        return Ok(Response::new_failure(
            StatusCode::BAD_REQUEST,
            vec![ResponseError {
                name: "forbidden".to_owned(),
                message: "relay_id does not match the registered relay_id".to_owned(),
            }],
            vec![],
        ));
    }

    let mut rng = StdRng::from_entropy();
    let keypair = Keypair::generate(&mut rng);

    let client = Client::new(
        &ConnectionOptions::new(
            "b7bbb0d762d747e486e20f72f0fb5a59", // TODO externalize
            AuthToken::new(body.relay_url.to_string())
                .as_jwt(&keypair)
                .unwrap(),
        )
        .with_address(format!("{}/rpc", body.relay_url)),
    )
    .unwrap(); // TODO handle
    let watch_register_response = client
        .watch_register_behalf(body.jwt.to_string())
        .await
        .unwrap(); // TODO handle

    let client_id = ClientId::from(claims.iss);
    debug!("register webhook client_id: {}", client_id.value());
    let relay_id = ClientId::from(watch_register_response.relay_id).into_value();

    state
        .registration2_store
        .upsert_registration(
            client_id.value(),
            body.tags.clone(),
            &body.relay_url,
            &relay_id,
        )
        .await?;

    state
        .registration2_cache
        .insert(client_id.into_value(), CachedRegistration2 {
            tags: body.tags.clone(),
            relay_url: body.relay_url.clone(),
            relay_id: relay_id.clone(),
        })
        .await;

    increment_counter!(state.metrics, register);

    Ok(Response::default())
}

// #[cfg(test)]
// mod test {
//     use super::*;

//     #[tokio::test]
//     async fn test_registration() {
//         // TODO mock state
//         // TODO mock relay
//         // TODO call handler function
//         // TODO check registered webhook in relay
//         // TODO check registered webhook in storage
//     }
// }
