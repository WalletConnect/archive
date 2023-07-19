use {
    super::StoreError,
    async_trait::async_trait,
    serde::{Deserialize, Serialize},
    std::sync::Arc,
    wither::{
        bson::{doc, oid::ObjectId},
        Model,
    },
};

#[derive(Clone, Debug, Model, Serialize, Deserialize, PartialEq, Eq)]
#[model(
    collection_name = "Registrations2",
    index(keys = r#"doc!{"client_id": 1}"#, options = r#"doc!{"unique": true}"#)
)]
pub struct Registration2 {
    /// MongoDB's default `_id` field.
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    /// The registered tags
    pub tags: Vec<u32>,
    /// The registered relay_url
    pub relay_url: Arc<str>,
    /// The registered relay_id
    pub relay_id: Arc<str>,
    /// The 'client_id' of the client owning the webhook.
    pub client_id: Arc<str>,
}

#[async_trait]
pub trait Registration2Store: 'static + Send + Sync {
    async fn upsert_registration(
        &self,
        client_id: &str,
        tags: Vec<u32>,
        relay_url: &str,
        relay_id: &str,
    ) -> Result<(), StoreError>;
    async fn get_registration(&self, client_id: &str) -> Result<Registration2, StoreError>;
}
