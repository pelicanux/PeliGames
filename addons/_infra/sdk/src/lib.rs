//! Shared contract for the launcher, package tooling and independent services.
#[cfg(feature = "packages")]
pub mod catalog;
#[cfg(feature = "packages")]
pub mod launch;
pub mod manifest;
#[cfg(feature = "packages")]
pub mod package;
pub mod protocol;
#[cfg(feature = "packages")]
pub mod registry;
#[cfg(feature = "runtime")]
pub mod runtime;
pub mod service;
#[cfg(feature = "packages")]
pub mod ui;

pub type Result<T> = std::result::Result<T, String>;

#[cfg(all(test, feature = "packages"))]
mod tests;

#[cfg(feature = "jobs")]
pub mod jobs;

pub mod files;
