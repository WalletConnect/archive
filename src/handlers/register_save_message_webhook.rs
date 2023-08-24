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
    reqwest::Url,
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
                message: "Authentication JWT iss does not match body JWT iss".to_owned(),
            }],
            vec![],
        ));
    }

    let mut rng = StdRng::from_entropy();
    let keypair = Keypair::generate(&mut rng);

    let relay_rpc_url = body
        .relay_url
        .parse::<Url>()
        .unwrap()
        .join("rpc")
        .unwrap()
        .to_string(); // TODO remove unwrap()
    let client = Client::new(
        &ConnectionOptions::new(
            "b7bbb0d762d747e486e20f72f0fb5a59", // TODO externalize
            AuthToken::new(body.relay_url.to_string())
                .as_jwt(&keypair)
                .unwrap(),
        )
        .with_address(relay_rpc_url),
    )
    .expect("Relay client setup should succeeed");
    let watch_register_response = client
        .watch_register_behalf(body.jwt.to_string())
        .await
        .unwrap(); // TODO remove unwrap()

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

#[cfg(test)]
mod test {
    use {
        super::*,
        crate::{
            config::Configuration,
            handlers::ResponseStatus,
            store::mocks::{
                messages::MockMessageStore,
                registrations::MockRegistrationStore,
                registrations2::MockRegistration2Store,
            },
        },
        chrono::Utc,
        relay_rpc::{
            auth::rand::rngs::OsRng,
            domain::{DecodedClientId, DidKey},
            rpc::{
                Params,
                Payload,
                Request,
                Response,
                SuccessfulResponse,
                WatchAction,
                WatchRegisterResponse,
                WatchStatus,
                WatchType,
                JSON_RPC_VERSION,
            },
        },
        wiremock::{
            http::Method,
            matchers::{method, path},
            Mock,
            MockServer,
            Request as WiremockRequest,
            ResponseTemplate,
        },
    };

    #[tokio::test]
    async fn test_registration() {
        let public_url = "http://localhost:3000";

        let relay_server = MockServer::start().await;
        let relay_url = relay_server.uri();

        Mock::given(method(Method::Post))
            .and(path(format!("/rpc")))
            .respond_with(|req: &WiremockRequest| {
                ResponseTemplate::new(StatusCode::OK).set_body_json(Response::Success(
                    SuccessfulResponse {
                        id: req.body_json::<Request>().unwrap().id,
                        jsonrpc: JSON_RPC_VERSION.clone(),
                        result: serde_json::to_value(WatchRegisterResponse {
                            relay_id: DidKey::from(DecodedClientId::from_key(
                                &Keypair::generate(&mut OsRng).public_key(),
                            )),
                        })
                        .unwrap(),
                    },
                ))
            })
            .mount(&relay_server)
            .await;

        let message_store = Arc::new(MockMessageStore::new());
        let registration_store = Arc::new(MockRegistrationStore::new());
        let registration2_store = Arc::new(MockRegistration2Store::new());
        let app_state = AppState::new(
            Configuration {
                port: 3000,
                public_url: public_url.to_owned(),
                log_level: "info".to_owned(),
                relay_url: relay_url.clone(),
                validate_signatures: false,
                is_test: true,
                otel_exporter_otlp_endpoint: None,
                telemetry_prometheus_port: None,
            },
            message_store,
            registration_store,
            registration2_store.clone(),
        )
        .unwrap();

        let keypair = Keypair::generate(&mut OsRng);
        let client_id = DecodedClientId::from_key(&keypair.public_key());
        let claims = JwtBasicClaims {
            iss: DidKey::from(client_id.clone()),
            aud: public_url.to_owned(),
            sub: "".to_owned(),
            iat: Utc::now().timestamp(),
            exp: None,
        };
        let jwt = claims.encode(&keypair).unwrap();
        let auth_bearer = AuthBearer(jwt);

        let tag = 4000;
        let tags = vec![tag];
        let watch_claims = WatchRegisterClaims {
            basic: JwtBasicClaims {
                iss: DidKey::from(DecodedClientId::from_key(&keypair.public_key())),
                aud: relay_url.to_owned(),
                sub: public_url.to_owned(),
                iat: Utc::now().timestamp(),
                exp: None,
            },
            act: WatchAction::Register,
            typ: WatchType::Publisher,
            whu: format!("{public_url}/v1/save-message-webhook"),
            tag: tags.clone(),
            sts: vec![WatchStatus::Accepted],
        };

        let register_payload = RegisterPayload {
            jwt: watch_claims.encode(&keypair).unwrap().into(),
            tags,
            relay_url: relay_url.clone().into(),
        };

        let response = handler(
            State(Arc::new(app_state)),
            auth_bearer,
            Json(register_payload),
        )
        .await
        .unwrap();
        assert_eq!(response.status, ResponseStatus::Success);
        assert!(response.status_code.is_success());

        let relay_requests = relay_server.received_requests().await.unwrap();
        assert_eq!(relay_requests.len(), 1);

        let relay_request = &relay_requests[0];
        let payload = relay_request.body_json::<Payload>().unwrap();
        payload.validate().unwrap();
        let Payload::Request(request) = payload else {
            panic!("payload not Request");
        };
        let Params::WatchRegister(watch_register) = request.params else {
            panic!("params not WatchRegister");
        };
        let watch_register_claims =
            WatchRegisterClaims::try_from_str(&watch_register.register_auth).unwrap();
        assert_eq!(watch_register_claims.act, WatchAction::Register);
        assert_eq!(watch_register_claims, watch_claims);

        let registration = registration2_store
            .registrations2
            .get(&client_id.to_string())
            .unwrap();
        assert_eq!(registration.relay_url, relay_url.into());
    }
}
