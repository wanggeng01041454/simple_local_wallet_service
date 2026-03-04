use axum::Router;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new().with_state(state)
}
