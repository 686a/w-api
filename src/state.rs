use mongodb::Database;

#[derive(Clone)]
pub struct GlobalState {
    pub remote_api_domain: String,
    pub db_client: Database,
}
