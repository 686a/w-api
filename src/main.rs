mod api;
mod errors;
mod state;

use axum::{
    Router,
    routing::{get, post},
};
use mongodb::{Client, bson::doc};
use std::net::SocketAddr;

use crate::state::GlobalState;

#[tokio::main]
async fn main() {
    let database_url =
        std::env::var("DATABASE_URL").expect("missing environment variable DATABASE_URL");
    let database_name =
        std::env::var("DATABASE_NAME").expect("missing environment variable DATABASE_NAME");
    let remote_api_domain =
        std::env::var("REMOTE_API_DOMAIN").expect("missing environment variable REMOTE_API_DOMAIN");

    let database_client = Client::with_uri_str(database_url).await.unwrap();

    let global_state = GlobalState {
        remote_api_domain,
        db_client: database_client.database(&database_name),
    };

    global_state
        .db_client
        .run_command(doc! { "ping": 1 })
        .await
        .expect("Failed to connect to database");
    println!("Database connected");

    let app = Router::new()
        .route("/", get(root))
        .route("/auth/login", post(api::auth::login::login))
        .route("/auth/remote/login", post(api::auth::remote::login::login))
        .with_state(global_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server started! http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Hello w-api"
}
