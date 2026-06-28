//! Adapters that implement the discovery domain's ports.
//!
//! Allowed deps: this feature's `domain`, `crate::platform::*`, third-party SDKs.
//! Forbidden: `application/`, `presentation/`, other features.

mod google_places_adapter;

pub use google_places_adapter::GooglePlacesAdapter;
