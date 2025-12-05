// Route definitions and handlers

use crate::handlers;
use crate::llm::agent::{EventSourcedAgent, ThreadStore};
use std::sync::Arc;
use uuid::Uuid;
use warp::Filter;

/// Configure all API routes with dependency injection
///
/// # Arguments
///
/// * `agent` - The event-sourced agent for processing messages
/// * `store` - The thread store for reading thread history
pub fn configure_routes(
    agent: Arc<EventSourcedAgent>,
    store: Arc<ThreadStore>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let api = warp::path("api").and(warp::path("v1"));

    // GET /threads/{threadId}
    let store_filter = warp::any().map(move || store.clone());
    let get_thread = api
        .and(warp::path("threads"))
        .and(warp::path::param::<Uuid>())
        .and(warp::path::end())
        .and(warp::get())
        .and(store_filter)
        .and_then(handlers::get_thread_handler);

    // POST /threads/{threadId}
    let agent_filter = warp::any().map(move || agent.clone());
    let post_message = api
        .and(warp::path("threads"))
        .and(warp::path::param::<Uuid>())
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .and(agent_filter)
        .and_then(handlers::send_message_handler);

    // Combine routes
    get_thread.or(post_message)
}
