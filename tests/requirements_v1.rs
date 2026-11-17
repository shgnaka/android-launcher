//! Requirements-first acceptance tests. Expectations come from the frozen v1 documents.
//! New behavior remains red until product implementation is changed; never weaken these tests.
#[path = "v1/catalog.rs"]
mod catalog;
#[path = "v1/home.rs"]
mod home;
#[path = "v1/persistence.rs"]
mod persistence;
#[cfg(feature = "ui")]
#[path = "v1/screens.rs"]
mod screens;
#[path = "v1/search.rs"]
mod search;
mod support;
#[path = "v1/usage.rs"]
mod usage;

#[cfg(feature = "ui")]
#[path = "v1/clock.rs"]
mod clock;
#[path = "v1/coordinates.rs"]
mod coordinates;
#[cfg(feature = "ui")]
#[path = "v1/media.rs"]
mod media;
#[cfg(feature = "ui")]
#[path = "v1/weather.rs"]
mod weather;
