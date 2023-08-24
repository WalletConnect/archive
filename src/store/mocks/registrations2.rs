use {
    crate::store::{
        registrations2::{Registration2, Registration2Store},
        StoreError,
    },
    async_trait::async_trait,
    moka::future::Cache,
    std::{fmt::Debug, sync::Arc},
};

#[derive(Debug)]
pub struct MockRegistration2Store {
    pub registrations2: Cache<String, Registration2>,
}

impl MockRegistration2Store {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            registrations2: Cache::builder().build(),
        }
    }
}

#[async_trait]
impl Registration2Store for MockRegistration2Store {
    async fn upsert_registration(
        &self,
        client_id: &str,
        tags: Vec<u32>,
        relay_url: &str,
        relay_id: &str,
    ) -> Result<(), StoreError> {
        let reg = Registration2 {
            id: None,
            tags,
            relay_url: Arc::from(relay_url),
            relay_id: Arc::from(relay_id),
            client_id: Arc::from(client_id),
        };

        self.registrations2.insert(client_id.to_string(), reg).await;
        Ok(())
    }

    async fn get_registration(&self, client_id: &str) -> Result<Registration2, StoreError> {
        self.registrations2
            .get(client_id)
            .ok_or(StoreError::NotFound(
                "registration2".to_string(),
                client_id.to_string(),
            ))
    }
}
