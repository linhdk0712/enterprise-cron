use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::state::AppState;

/// Server-Sent Events handler for real-time updates
#[tracing::instrument(skip(state))]
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sse_tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|msg| {
        match msg {
            Ok(event) => {
                let json = serde_json::to_string(&event).unwrap_or_default();
                Ok(Event::default().data(json))
            }
            Err(_) => {
                // Lagged behind, send a reconnect event
                Ok(Event::default().event("reconnect"))
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
