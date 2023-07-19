use {
    crate::{context::server::ServerContext, get_client_jwt, RELAY_HTTP_URL},
    axum::http,
    gilgamesh::{handlers::register::RegisterPayload, store::registrations::Registration},
    std::sync::Arc,
    test_context::test_context,
};

#[test_context(ServerContext)]
#[tokio::test]
async fn test_register(ctx: &mut ServerContext) {
    let (jwt, client_id, _) = get_client_jwt(ctx.server.public_url.clone());

    let payload = RegisterPayload {
        tags: vec![Arc::from("4000"), Arc::from("5***")],
        relay_url: Arc::from(RELAY_HTTP_URL),
    };

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/register", ctx.server.public_url))
        .json(&payload)
        .header(http::header::AUTHORIZATION, format!("Bearer {jwt}"))
        .send()
        .await
        .expect("Call failed");

    assert!(
        response.status().is_success(),
        "Response was not successful: {:?} - {:?}",
        response.status(),
        response.text().await
    );

    assert!(ctx
        .registration_store
        .registrations
        .get(client_id.value().as_ref())
        .is_some())
}

#[test_context(ServerContext)]
#[tokio::test]
async fn test_get_registration(ctx: &mut ServerContext) {
    let (jwt, client_id, _) = get_client_jwt(ctx.server.public_url.clone());

    let tags = vec![Arc::from("4000"), Arc::from("5***")];
    let registration = Registration {
        id: None,
        client_id: client_id.clone().into_value(),
        tags: tags.clone(),
        relay_url: Arc::from(RELAY_HTTP_URL),
    };

    ctx.registration_store
        .registrations
        .insert(client_id.to_string(), registration)
        .await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/register", ctx.server.public_addr))
        .header(http::header::AUTHORIZATION, format!("Bearer {jwt}"))
        .send()
        .await
        .expect("Call failed");

    assert!(
        response.status().is_success(),
        "Response was not successful: {:?} - {:?}",
        response.status(),
        response.text().await
    );

    assert!(response
        .headers()
        .contains_key("Access-Control-Allow-Origin"));
    let allowed_origins = response
        .headers()
        .get("Access-Control-Allow-Origin")
        .unwrap();
    assert_eq!(allowed_origins.to_str().unwrap(), "*");

    let payload: RegisterPayload = response.json().await.unwrap();
    assert_eq!(payload.tags, tags);
    assert_eq!(payload.relay_url.as_ref(), RELAY_HTTP_URL);
}
