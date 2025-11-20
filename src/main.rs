mod handlers;
mod models;
mod routes;
mod sse;

use routes::configure_routes;

#[tokio::main]
async fn main() {
    let routes = configure_routes();

    println!("Starting server on http://127.0.0.1:3030");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
