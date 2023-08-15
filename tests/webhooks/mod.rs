use {
    crate::{context::store::ServerStoreContext, get_client_jwt, RELAY_HTTP_URL},
    axum::http,
    chrono::Utc,
    gilgamesh::{
        handlers::register_save_message_webhook::RegisterPayload,
        store::messages::MessagesStore,
    },
    log::debug,
    relay_client::{http::Client, ConnectionOptions},
    relay_rpc::{
        auth::AuthToken,
        domain::{DidKey, Topic},
        jwt::{JwtBasicClaims, VerifyableClaims},
        rpc::{WatchAction, WatchRegisterClaims, WatchStatus, WatchType},
    },
    std::{sync::Arc, time::Duration},
    test_context::test_context,
};

// ==== TODO

// == Integration tests, gate deployment
// Use prod/staging/dev/localhost instead of deploying history server here
// Use API to get message instead of calling the store
// Use JS SDK

// == Unit tests
// Don't use relay
// Call webhook registration & handler function directly
// Access message via get_message handler function directly

#[test_context(ServerStoreContext)]
#[tokio::test]
#[cfg_attr(not(feature = "storage-tests"), ignore)]
async fn test_webhooks_registration(ctx: &mut ServerStoreContext) {
    let [(client1_jwt, client1_id, client1_keypair)
        //, (client2_jwt, client2_id, client2_keypair)
    ] =
        [get_client_jwt(ctx.server.public_url.clone())
        //, get_client_jwt()
        ];

    // Register watcher
    let tag = 4000;
    {
        let tags = vec![tag];
        let relay_url = Arc::from(RELAY_HTTP_URL);

        let iat = Utc::now();
        let jwt = WatchRegisterClaims {
            basic: JwtBasicClaims {
                iat: iat.timestamp(),
                exp: Some(
                    (iat + chrono::Duration::from_std(Duration::from_secs(60 * 60)).unwrap())
                        .timestamp(),
                ),
                iss: DidKey::try_from(client1_id.clone()).unwrap(),
                aud: RELAY_HTTP_URL.to_owned(),
                sub: ctx.server.public_url.clone(),
            },
            typ: WatchType::Publisher,
            act: WatchAction::Register,
            whu: format!("{}/v1/save-message-webhook", ctx.server.public_url),
            tag: tags.clone(),
            sts: vec![WatchStatus::Accepted],
        }
        .encode(&client1_keypair)
        .unwrap()
        .into();

        let payload = RegisterPayload {
            jwt,
            tags,
            relay_url,
        };

        let client = reqwest::Client::new();
        let response = client
            .post(format!(
                "{}/v1/register-save-message-webhook",
                ctx.server.public_url
            ))
            .json(&payload)
            .header(http::header::AUTHORIZATION, format!("Bearer {client1_jwt}"))
            .send()
            .await
            .expect("Call failed");

        assert!(
            response.status().is_success(),
            "Response was not successful: {:?} - {:?}",
            response.status(),
            response.text().await
        );

        // assert!((Arc::new(ctx.storage.store) as Registration2StorageArc)
        //     .get_registration(client1_id.value().as_ref())
        //     .await
        //     .is_ok());
    }

    // tokio::time::sleep(Duration::from_secs(2)).await;

    let topic = Topic::generate();
    let message: Arc<str> = Arc::from("Hello WalletConnect!");
    // Publish message
    {
        let client = Client::new(
            &ConnectionOptions::new(
                "b7bbb0d762d747e486e20f72f0fb5a59", // TODO externalize
                AuthToken::new(RELAY_HTTP_URL)
                    .as_jwt(&client1_keypair)
                    .unwrap(),
            )
            .with_address(format!("{RELAY_HTTP_URL}/rpc")),
        )
        .unwrap();
        client
            .publish(
                topic.clone(),
                message.clone(),
                tag,
                Duration::from_secs(30),
                false,
            )
            .await
            .unwrap();
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check message in store
    {
        debug!("checking message in topic {}", topic);
        let result = ctx
            .storage
            .store
            .get_messages_after(topic.as_ref(), None, 1)
            .await
            .unwrap();

        assert_eq!(result.messages.len(), 1, "check result length");

        assert_eq!(result.messages.first().unwrap().message, message,);

        assert_eq!(result.next_id, None, "Check next_id");
    }
}

// #[test_context(ServerContext)]
// #[tokio::test]
// async fn test_get_registration(ctx: &mut ServerContext) {
//     let (jwt, client_id) = get_client_jwt();

//     let tags = vec![Arc::from("4000"), Arc::from("5***")];
//     let registration = Registration {
//         id: None,
//         client_id: client_id.clone().into_value(),
//         tags: tags.clone(),
//         relay_url: Arc::from(RELAY_URL),
//     };

//     ctx.server
//         .registration_store
//         .registrations
//         .insert(client_id.to_string(), registration)
//         .await;

//     let client = reqwest::Client::new();
//     let response = client
//         .get(format!("http://{}/register", ctx.server.public_addr))
//         .header(http::header::AUTHORIZATION, format!("Bearer {jwt}"))
//         .send()
//         .await
//         .expect("Call failed");

//     assert!(
//         response.status().is_success(),
//         "Response was not successful: {:?} - {:?}",
//         response.status(),
//         response.text().await
//     );

//     assert!(response
//         .headers()
//         .contains_key("Access-Control-Allow-Origin"));
//     let allowed_origins = response
//         .headers()
//         .get("Access-Control-Allow-Origin")
//         .unwrap();
//     assert_eq!(allowed_origins.to_str().unwrap(), "*");

//     let payload: RegisterPayload = response.json().await.unwrap();
//     assert_eq!(payload.tags, tags);
//     assert_eq!(payload.relay_url.as_ref(), RELAY_URL);
// }
