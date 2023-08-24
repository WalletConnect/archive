use {
    crate::{context::server_full::ServerFullContext, get_client_jwt},
    axum::http,
    chrono::Utc,
    gilgamesh::handlers::{
        get_messages::GetMessagesResponse,
        register_save_message_webhook::RegisterPayload,
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

#[test_context(ServerFullContext)]
#[tokio::test]
#[cfg_attr(not(feature = "relay-tests"), ignore)]
async fn test_webhooks_registration(ctx: &mut ServerFullContext) {
    let (client1_jwt, client1_id, client1_keypair) = get_client_jwt(ctx.server_url.clone());

    // Register watcher
    let tag = 4000;
    {
        let tags = vec![tag];
        let relay_url = Arc::from(ctx.relay_url.clone());

        let iat = Utc::now();
        let jwt = WatchRegisterClaims {
            basic: JwtBasicClaims {
                iat: iat.timestamp(),
                exp: Some(
                    (iat + chrono::Duration::from_std(Duration::from_secs(60 * 60)).unwrap())
                        .timestamp(),
                ),
                iss: DidKey::try_from(client1_id.clone()).unwrap(),
                aud: ctx.relay_url.to_owned(),
                sub: ctx.server_url.clone(),
            },
            typ: WatchType::Publisher,
            act: WatchAction::Register,
            whu: format!("{}/v1/save-message-webhook", ctx.server_url),
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
                ctx.server_url
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
    }

    let topic = Topic::generate();
    let message: Arc<str> = Arc::from("Hello WalletConnect!");
    // Publish message
    {
        let client = Client::new(
            &ConnectionOptions::new(
                "b7bbb0d762d747e486e20f72f0fb5a59", // TODO externalize
                AuthToken::new(ctx.relay_url.clone())
                    .as_jwt(&client1_keypair)
                    .unwrap(),
            )
            .with_address(format!("{}/rpc", ctx.relay_url)),
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
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/messages", ctx.server_url))
            .query(&[("topic", topic)])
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

        let response = response.json::<GetMessagesResponse>().await.unwrap();

        assert_eq!(response.messages.len(), 1, "check result length");
        assert_eq!(response.messages.first().unwrap().message, message,);
        assert_eq!(response.next_id, None, "Check next_id");
    }
}
