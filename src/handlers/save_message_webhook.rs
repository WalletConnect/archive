use {
    crate::{
        error,
        handlers::{Response, ResponseError},
        increment_counter,
        log::prelude::*,
        state::{AppState, CachedRegistration2},
        store::{registrations2::Registration2, StoreError},
    },
    axum::{extract::State as StateExtractor, Json},
    hyper::StatusCode,
    relay_rpc::{
        domain::{ClientId, DecodedClientId},
        jwt::VerifyableClaims,
        rpc::{msg_id::get_message_id, WatchEventClaims, WatchWebhookPayload},
    },
    std::sync::Arc,
};

/// Webhooks 2.0 webhook
pub async fn handler(
    StateExtractor(state): StateExtractor<Arc<AppState>>,
    Json(payload): Json<WatchWebhookPayload>,
) -> error::Result<Response> {
    debug!("Received webhook: {:?}", payload);

    increment_counter!(state.metrics, received_items);

    let jwt = payload.event_auth;
    let claims = WatchEventClaims::try_from_str(&jwt)?;
    claims.verify_basic(&state.auth_aud, None)?;
    let client_id =
        ClientId::from(DecodedClientId::try_from_did_key(&claims.basic.sub).unwrap()).into_value();
    debug!("webhook sub client_id: {client_id}");
    let evt = claims.evt;

    let registration = if let Some(registration) = state
        .registration2_cache
        .get(client_id.as_ref()) // TODO should we key on webhookId instead?
        .map(|r| Registration2 {
            id: None,
            tags: r.tags,
            relay_url: r.relay_url,
            relay_id: r.relay_id,
            client_id: client_id.clone(),
        }) {
        debug!("loaded registration from cache");
        increment_counter!(state.metrics, cached_registrations);
        registration
    } else {
        debug!("loading registration from database");
        let registration = match state
            .registration2_store
            .get_registration(client_id.as_ref())
            .await
        {
            Ok(registration) => registration,
            Err(StoreError::NotFound(_, _)) => {
                debug!("registration not found, returning 2xx status code to webhook");
                return Ok(Response::default());
            }
            Err(e) => return Err(e.into()),
        };

        state
            .registration2_cache
            .insert(client_id.clone(), CachedRegistration2 {
                tags: registration.tags.clone(),
                relay_url: registration.relay_url.clone(),
                relay_id: registration.relay_id.clone(),
            })
            .await;

        increment_counter!(state.metrics, fetched_registrations);
        registration
    };

    // TODO use global relay_id list for authentication
    let relay_id = ClientId::from(claims.basic.iss).into_value(); // Authenticated by `WatchEventClaims::try_from_str()` above
    if registration.relay_id != relay_id {
        return Ok(Response::new_failure(
            StatusCode::FORBIDDEN,
            vec![ResponseError {
                name: "forbidden".to_owned(),
                message: "relay_id does not match the registered relay_id".to_owned(),
            }],
            vec![],
        ));
    }

    // TODO Should probably remove this as there could be a race condition where new
    // tags get added to the relay and a webhook triggered before the registration
    // is updated
    if !registration.tags.contains(&evt.tag) {
        warn!(
            "received message with tag {} but this does not match registration tags {:?}",
            evt.tag, registration.tags
        );
    }

    debug!("storing message to topic {}", evt.topic);

    state
        .message_store
        .upsert_message(
            None, // https://walletconnect.slack.com/archives/C04CKNV4GN8/p1688127376863359
            &client_id,
            evt.topic.as_ref(),
            &get_message_id(&evt.message),
            &evt.message,
        )
        .await?;

    debug!("message stored, sending ack");

    increment_counter!(state.metrics, stored_items);

    Ok(Response::default())
}

// #[cfg(test)]
// mod test {
//     use super::*;

//     #[tokio::test]
//     async fn test_registration() {
//         // TODO mock state
//         // TODO mock registration state w/ relay ID
//         // TODO call handler function w/ message webhook
//         // TODO check message in storage
//     }
// }
