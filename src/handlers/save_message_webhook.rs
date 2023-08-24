use {
    crate::{
        error,
        handlers::{Response, ResponseError},
        increment_counter,
        log::prelude::*,
        state::{AppState, CachedRegistration2},
        store::{registrations2::Registration2, StoreError},
    },
    axum::{extract::State, Json},
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
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WatchWebhookPayload>,
) -> error::Result<Response> {
    debug!("Received webhook: {:?}", payload);

    increment_counter!(state.metrics, received_items);

    let jwt = payload.event_auth;
    let claims = WatchEventClaims::try_from_str(&jwt)?;
    claims.verify_basic(&state.auth_aud, None)?;
    let client_id =
        ClientId::from(DecodedClientId::try_from_did_key(&claims.basic.sub).unwrap()).into_value();
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

#[cfg(test)]
mod test {
    use {
        super::*,
        crate::{
            config::Configuration,
            handlers::ResponseStatus,
            store::{
                messages::MessagesStore,
                mocks::{
                    messages::MockMessageStore,
                    registrations::MockRegistrationStore,
                    registrations2::MockRegistration2Store,
                },
                registrations2::Registration2Store,
            },
        },
        chrono::Utc,
        relay_rpc::{
            auth::{ed25519_dalek::Keypair, rand::rngs::OsRng},
            domain::{DidKey, Topic},
            jwt::JwtBasicClaims,
            rpc::{WatchAction, WatchEventPayload, WatchType},
        },
    };

    #[tokio::test]
    async fn test_webhook() {
        let public_url = "http://localhost:3000";
        let relay_url = "http://localhost:3001";

        let message_store = Arc::new(MockMessageStore::new());
        let registration_store = Arc::new(MockRegistrationStore::new());
        let registration2_store = Arc::new(MockRegistration2Store::new());
        let app_state = AppState::new(
            Configuration {
                port: 3000,
                public_url: public_url.to_owned(),
                log_level: "info".to_owned(),
                relay_url: relay_url.to_owned(),
                validate_signatures: false,
                is_test: true,
                otel_exporter_otlp_endpoint: None,
                telemetry_prometheus_port: None,
            },
            message_store.clone(),
            registration_store,
            registration2_store.clone(),
        )
        .unwrap();

        let client_keypair = Keypair::generate(&mut OsRng);
        let client_id = DecodedClientId::from_key(&client_keypair.public_key());
        let relay_keypair = Keypair::generate(&mut OsRng);
        let relay_id = DecodedClientId::from_key(&relay_keypair.public_key());

        let tag = 4000;
        registration2_store
            .upsert_registration(
                &client_id.to_string(),
                vec![tag],
                relay_url,
                &relay_id.to_string(),
            )
            .await
            .unwrap();

        let topic = Topic::generate().to_string();
        let message = "test-message";
        let event_auth = WatchEventClaims {
            basic: JwtBasicClaims {
                iss: DidKey::from(DecodedClientId::from_key(&relay_keypair.public_key())),
                aud: public_url.to_owned(),
                sub: client_id.to_did_key(),
                iat: Utc::now().timestamp(),
                exp: None,
            },
            act: WatchAction::WatchEvent,
            typ: WatchType::Publisher,
            whu: format!("{public_url}/v1/save-message-webhook"),
            evt: WatchEventPayload {
                status: relay_rpc::rpc::WatchStatus::Accepted,
                topic: topic.clone().into(),
                message: message.to_owned().into(),
                published_at: Utc::now().timestamp(),
                tag,
            },
        }
        .encode(&relay_keypair)
        .unwrap();
        let webhook_payload = WatchWebhookPayload { event_auth };

        let response = handler(State(Arc::new(app_state)), Json(webhook_payload))
            .await
            .unwrap();
        assert_eq!(response.status, ResponseStatus::Success);
        assert!(response.status_code.is_success());

        let messages = message_store
            .get_messages_after(&topic, None, 2)
            .await
            .unwrap();
        assert_eq!(messages.messages.len(), 1);
        let message_text = message;
        let message = messages.messages.first().unwrap();
        assert_eq!(message.topic, topic.into());
        assert_eq!(message.message, message_text.into());
        assert_eq!(message.message_id, get_message_id(message_text).into());
        assert_eq!(message.client_id, client_id.to_string().into());
    }

    // TODO test wrong aud in WatchEvent payload
    // TODO test wrong iss (not relay) in WatchEvent payload
}
