//! AFMF settings and presentation integration, independent of the launcher.
pub mod compatibility;
pub mod download;
pub mod management;
pub mod releases;
pub mod runtime;
mod settings;
pub mod support;
pub use settings::Settings;
#[cfg(test)]
mod management_tests;
