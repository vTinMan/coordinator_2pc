pub mod handler;
pub mod registrator;
pub mod state_tracker;

use axum::{ routing::{get, post}, Router };
use crate::state_tracker::{run_expiration_tracker};
use crate::registrator::{load_service_configs};

static SERVICE_CONFIG_FILE: &str = "./example_services.yml";

#[tokio::main]
async fn main() {
    env_logger::init();
    match load_service_configs(SERVICE_CONFIG_FILE) {
        Err(e) => panic!("Loading a service configs failed: {}", e),
        _ => ()
    };
    tokio::spawn(run_expiration_tracker());
    let app = Router::new()
        .route("/version", get(version))
        .route("/transactions", post(handler::create))
        .route("/transactions/:uid/confirm/:service", post(handler::confirm))
        .route("/transactions/:uid/abort/:service", post(handler::abort));

    axum::Server::bind(&"0.0.0.0:8080".parse().unwrap())
        .serve(app.into_make_service()).await.unwrap();
}

async fn version() -> &'static str {
    "{\
        'name': 'coordinator'\
        'version': '0.1.0'\
     }"
}
