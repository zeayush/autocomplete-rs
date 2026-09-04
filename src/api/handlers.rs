//! axum handlers. Keep them thin — pure translation between HTTP and Engine.

use crate::{Engine, Result};
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

/// Beyond this the fuzzy walk stops pruning usefully and starts scanning the
/// whole trie, which would blow the latency budget.
const MAX_TYPO_BUDGET: u32 = 2;

/// Bounds the response size so one caller cannot ask for the whole corpus.
const MAX_K: usize = 100;

#[derive(Serialize)]
pub struct Hit {
    pub term: String,
    pub score: u64,
    /// Only populated when typo > 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<u32>,
}

#[derive(Serialize)]
pub struct Health {
    pub status: &'static str,
    pub tenants: Vec<TenantHealth>,
}

#[derive(Serialize)]
pub struct TenantHealth {
    pub tenant: String,
    #[serde(flatten)]
    pub stats: crate::trie::TrieStats,
}

pub async fn health(State(engine): State<Arc<Engine>>) -> impl IntoResponse {
    Json(Health {
        status: "ok",
        tenants: engine
            .stats()
            .into_iter()
            .map(|(tenant, stats)| TenantHealth { tenant, stats })
            .collect(),
    })
}

pub async fn index(
    State(engine): State<Arc<Engine>>,
    Path(tenant): Path<String>,
    Json(body): Json<IndexBody>,
) -> Result<impl IntoResponse> {
    engine.index(&tenant, &body.term, body.weight).await?;
    // 202: the trie is already updated, but durability is still in flight.
    Ok(StatusCode::ACCEPTED)
}

pub async fn query(
    State(engine): State<Arc<Engine>>,
    Path(tenant): Path<String>,
    Query(params): Query<QueryParams>,
) -> Result<impl IntoResponse> {
    if params.typo > MAX_TYPO_BUDGET {
        return Err(crate::Error::Invalid(format!(
            "typo budget must be 0..={MAX_TYPO_BUDGET}, got {}",
            params.typo
        )));
    }
    let k = params.k.min(MAX_K);

    let hits: Vec<Hit> = if params.typo == 0 {
        engine
            .query(&tenant, &params.prefix, k)
            .into_iter()
            .map(|(term, score)| Hit { term, score, distance: None })
            .collect()
    } else {
        engine
            .query_fuzzy(&tenant, &params.prefix, params.typo, k)
            .into_iter()
            .map(|(term, score, distance)| Hit { term, score, distance: Some(distance) })
            .collect()
    };

    Ok(Json(hits))
}

pub async fn delete(
    State(engine): State<Arc<Engine>>,
    Path((tenant, term)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    engine.delete(&tenant, &term).await?;
    Ok(StatusCode::NO_CONTENT)
}
