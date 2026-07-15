//! axum handlers. Keep them thin — pure translation between HTTP and Engine.

use crate::Engine;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct IndexBody {
    pub term: String,
    /// Defaults to 1 if omitted.
    #[serde(default = "default_weight")]
    pub weight: u64,
}

fn default_weight() -> u64 { 1 }

#[derive(Deserialize)]
pub struct QueryParams {
    pub prefix: String,
    #[serde(default = "default_k")]
    pub k: usize,
    /// 0 = exact, 1 or 2 = fuzzy Levenshtein budget.
    #[serde(default)]
    pub typo: u32,
}

fn default_k() -> usize { 10 }

#[derive(Serialize)]
pub struct Hit {
    pub term: String,
    pub score: u64,
    /// Only populated when typo > 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<u32>,
}

pub async fn health() -> impl IntoResponse {
    // HINT: return `Json(json!({"status": "ok"}))` or a static &str.
    todo!()
}

pub async fn index(
    State(engine): State<Arc<Engine>>,
    Path(tenant): Path<String>,
    Json(body): Json<IndexBody>,
) -> impl IntoResponse {
    // HINT: engine.index(&tenant, &body.term, body.weight).await
    //       → map Ok to StatusCode::ACCEPTED
    //       → map Err to (StatusCode::BAD_REQUEST, message)
    let _ = (engine, tenant, body);
    (StatusCode::NOT_IMPLEMENTED, "todo")
}

pub async fn query(
    State(engine): State<Arc<Engine>>,
    Path(tenant): Path<String>,
    Query(params): Query<QueryParams>,
) -> impl IntoResponse {
    // HINT: branch on params.typo:
    //   0 → engine.query(...) → Vec<(term, score)>
    //   n → engine.query_fuzzy(..., n, k) → Vec<(term, score, dist)>
    // Serialize to Vec<Hit> and return Json.
    let _ = (engine, tenant, params);
    (StatusCode::NOT_IMPLEMENTED, "todo")
}

pub async fn delete(
    State(engine): State<Arc<Engine>>,
    Path((tenant, term)): Path<(String, String)>,
) -> impl IntoResponse {
    // HINT: engine.delete(&tenant, &term).await → 204 No Content on success.
    let _ = (engine, tenant, term);
    (StatusCode::NOT_IMPLEMENTED, "todo")
}
