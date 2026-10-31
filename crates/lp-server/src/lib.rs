//! The LearnPuppy HTTP server: axum router, auth, error envelope, SSE, OpenAPI, embedded SPA.
//! The `learnpuppy` binary (`main.rs`) is a thin CLI over this library.

pub mod app;
pub mod assets;
pub mod auth;
pub mod cli;
pub mod directory;
pub mod doctor;
pub mod dto;
pub mod engine;
pub mod error;
pub mod openapi;
pub mod routes;
pub mod seed;
pub mod serve;
pub mod sse;
