//! `GooglePlacesAdapter` — implements `DiscoveryPort` against the new
//! Google Maps Places API (`places:searchText`).
//!
//! Endpoint: `POST https://places.googleapis.com/v1/places:searchText`
//! Auth: `X-Goog-Api-Key` header.
//! Field mask is required by the new API — we ask only for the columns we
//! actually map. Smaller mask = cheaper call (Google bills per field group).
//!
//! Error mapping:
//!   * 401/403 → `DiscoveryError::MissingKey`
//!   * 429     → `DiscoveryError::QuotaExhausted`
//!   * 5xx     → `DiscoveryError::Upstream(body)`
//!   * other non-2xx → `DiscoveryError::Upstream(body)`
//!   * transport failure → `DiscoveryError::Network(message)`

use std::time::Duration;

use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

use crate::features::discovery::domain::{Candidate, DiscoveryError, DiscoveryPort, NicheQuery};

const SEARCH_TEXT_URL: &str = "https://places.googleapis.com/v1/places:searchText";
const FIELD_MASK: &str = "places.id,places.displayName,places.websiteUri,places.internationalPhoneNumber,places.formattedAddress,places.rating,places.userRatingCount";
const USER_AGENT_VALUE: &str = "ApolloBot/0.1";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Adapter wired into `DiscoverLeads` via `configure.rs`.
///
/// `key = None` is the stub mode used when `GOOGLE_MAPS_API_KEY` is unset at
/// boot: every `find` call returns `DiscoveryError::MissingKey`, but the rest
/// of the system boots and stays usable.
#[derive(Clone)]
pub struct GooglePlacesAdapter {
    client: Client,
    key: Option<String>,
}

impl GooglePlacesAdapter {
    /// Read `GOOGLE_MAPS_API_KEY` and build a configured adapter.
    ///
    /// Returns `DiscoveryError::MissingKey` if the env var is unset or empty,
    /// or `DiscoveryError::Network` if the `reqwest` client fails to build
    /// (TLS init, etc.).
    pub fn from_env() -> Result<Self, DiscoveryError> {
        let key = std::env::var("GOOGLE_MAPS_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .ok_or(DiscoveryError::MissingKey)?;
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(USER_AGENT_VALUE)
            .build()
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;
        Ok(Self {
            client,
            key: Some(key),
        })
    }

    /// Build a key-less adapter whose `find` always returns
    /// `DiscoveryError::MissingKey`. Used by `configure()` when
    /// `GOOGLE_MAPS_API_KEY` is unset so the rest of the app still boots.
    pub fn stub() -> Self {
        // The client is unused in stub mode; build a minimal default so we
        // never `unwrap` on it.
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(USER_AGENT_VALUE)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client, key: None }
    }

    fn build_headers(&self, key: &str) -> Result<HeaderMap, DiscoveryError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert("X-Goog-FieldMask", HeaderValue::from_static(FIELD_MASK));
        let mut key_value = HeaderValue::from_str(key)
            .map_err(|e| DiscoveryError::Upstream(format!("invalid api key header: {e}")))?;
        key_value.set_sensitive(true);
        headers.insert("X-Goog-Api-Key", key_value);
        Ok(headers)
    }
}

#[async_trait]
impl DiscoveryPort for GooglePlacesAdapter {
    async fn find(&self, query: &NicheQuery) -> Result<Vec<Candidate>, DiscoveryError> {
        let key = self.key.as_ref().ok_or(DiscoveryError::MissingKey)?;

        // Google Places returns up to 20 results per page. V1 is single-page,
        // so cap the request at 20 and rely on `query.limit` to truncate.
        let max_result_count = query.limit.clamp(1, 20);
        let text_query = format!("{} in {}, {}", query.niche, query.city, query.country);

        let body = SearchTextRequest {
            text_query: &text_query,
            max_result_count,
        };

        let headers = self.build_headers(key)?;
        let response = self
            .client
            .post(SEARCH_TEXT_URL)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let snippet = response
                .text()
                .await
                .unwrap_or_else(|e| format!("<failed to read body: {e}>"));
            return Err(map_status_error(status, snippet));
        }

        let parsed: SearchTextResponse = response
            .json()
            .await
            .map_err(|e| DiscoveryError::Upstream(format!("decode error: {e}")))?;

        let limit = query.limit as usize;
        let candidates: Vec<Candidate> = parsed
            .places
            .unwrap_or_default()
            .into_iter()
            .map(place_to_candidate)
            .take(limit)
            .collect();

        Ok(candidates)
    }
}

fn map_status_error(status: StatusCode, body: String) -> DiscoveryError {
    match status.as_u16() {
        401 | 403 => DiscoveryError::MissingKey,
        429 => DiscoveryError::QuotaExhausted,
        500..=599 => DiscoveryError::Upstream(body),
        _ => DiscoveryError::Upstream(body),
    }
}

fn place_to_candidate(place: Place) -> Candidate {
    let name = place
        .display_name
        .as_ref()
        .map(|d| d.text.clone())
        .unwrap_or_default();
    let url = place.website_uri.clone();
    let rating = place.rating;
    // Confidence rubric per the spec.
    let confidence = if !name.is_empty() && url.is_some() && rating.is_some() {
        0.9
    } else if !name.is_empty() && url.is_some() {
        0.7
    } else {
        0.5
    };
    Candidate {
        name,
        url,
        phone: place.international_phone_number,
        whatsapp: None,
        address: place.formatted_address,
        rating,
        review_count: place.user_rating_count,
        source: "google_places".to_string(),
        source_id: place.id,
        confidence,
    }
}

// --- wire types --------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchTextRequest<'a> {
    text_query: &'a str,
    max_result_count: u32,
}

#[derive(Debug, Deserialize)]
struct SearchTextResponse {
    #[serde(default)]
    places: Option<Vec<Place>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Place {
    id: String,
    #[serde(default)]
    display_name: Option<DisplayName>,
    #[serde(default)]
    website_uri: Option<String>,
    #[serde(default)]
    international_phone_number: Option<String>,
    #[serde(default)]
    formatted_address: Option<String>,
    #[serde(default)]
    rating: Option<f32>,
    #[serde(default)]
    user_rating_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct DisplayName {
    text: String,
    #[serde(default)]
    #[allow(dead_code)]
    language_code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place_with(id: &str, name: Option<&str>, url: Option<&str>, rating: Option<f32>) -> Place {
        Place {
            id: id.to_string(),
            display_name: name.map(|t| DisplayName {
                text: t.to_string(),
                language_code: None,
            }),
            website_uri: url.map(|s| s.to_string()),
            international_phone_number: None,
            formatted_address: None,
            rating,
            user_rating_count: None,
        }
    }

    #[test]
    fn confidence_high_when_name_url_rating_all_present() {
        let c = place_to_candidate(place_with(
            "x",
            Some("Acme"),
            Some("https://acme.test"),
            Some(4.6),
        ));
        assert!((c.confidence - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn confidence_medium_when_name_and_url_only() {
        let c = place_to_candidate(place_with(
            "x",
            Some("Acme"),
            Some("https://acme.test"),
            None,
        ));
        assert!((c.confidence - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn confidence_low_otherwise() {
        let c = place_to_candidate(place_with("x", Some("Acme"), None, Some(4.0)));
        assert!((c.confidence - 0.5).abs() < f32::EPSILON);
        let c = place_to_candidate(place_with("x", None, None, None));
        assert!((c.confidence - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn maps_status_codes_to_expected_errors() {
        assert!(matches!(
            map_status_error(StatusCode::UNAUTHORIZED, String::new()),
            DiscoveryError::MissingKey
        ));
        assert!(matches!(
            map_status_error(StatusCode::FORBIDDEN, String::new()),
            DiscoveryError::MissingKey
        ));
        assert!(matches!(
            map_status_error(StatusCode::TOO_MANY_REQUESTS, String::new()),
            DiscoveryError::QuotaExhausted
        ));
        assert!(matches!(
            map_status_error(StatusCode::BAD_GATEWAY, "x".into()),
            DiscoveryError::Upstream(_)
        ));
        assert!(matches!(
            map_status_error(StatusCode::BAD_REQUEST, "x".into()),
            DiscoveryError::Upstream(_)
        ));
    }

    #[tokio::test]
    async fn stub_returns_missing_key() {
        let adapter = GooglePlacesAdapter::stub();
        let q = NicheQuery {
            niche: "x".into(),
            city: "y".into(),
            country: "AE".into(),
            limit: 5,
        };
        let err = adapter.find(&q).await.unwrap_err();
        assert!(matches!(err, DiscoveryError::MissingKey));
    }
}
