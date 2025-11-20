// Route definitions and handlers

use crate::handlers;
use uuid::Uuid;
use warp::Filter;

pub fn configure_routes() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
{
    let api = warp::path("api").and(warp::path("v1"));

    // GET /threads/{threadId}
    let get_thread = api
        .and(warp::path("threads"))
        .and(warp::path::param::<Uuid>())
        .and(warp::path::end())
        .and(warp::get())
        .and_then(handlers::get_thread_handler);

    // POST /threads/{threadId}
    let post_message = api
        .and(warp::path("threads"))
        .and(warp::path::param::<Uuid>())
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .and_then(handlers::send_message_handler);

    // Combine routes
    get_thread.or(post_message)
}
