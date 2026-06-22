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
    let bind_addr = std::env::var("BIND_ADDR").expect("missing environment variable BIND_ADDR");
    let addr: SocketAddr = bind_addr
        .parse()
        .expect("invalid environment variable BIND_ADDR");

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
        .route("/bbs/boards", get(api::bbs::boards))
        .route("/bbs/posts", get(api::bbs::posts))
        .route("/bbs/posts/{cid}", get(api::bbs::post))
        .route("/bbs/posts/{cid}/text", get(api::bbs::post_text))
        .route(
            "/bbs/posts/{cid}/attachments/{index}",
            get(api::bbs::attachment),
        )
        .route("/info-service/pages", get(api::info_service::pages))
        .route("/info-service/pages/{key}", get(api::info_service::page))
        .with_state(global_state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind server address");
    let local_addr = listener.local_addr().expect("failed to read local address");
    println!("Server started! http://{}", local_addr);

    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Hello w-api"
}
