//! Wire types (docs/web/03 §DTO rule): camelCase, `ToSchema`, built from service types with
//! `From` impls. Service and core types are never serialized directly.

pub mod chat;
pub mod course;
pub mod digest;
pub mod events;
pub mod graph;
pub mod learning;
pub mod review;
pub mod search;
pub mod settings;
pub mod system;
pub mod workspace;

pub mod mutations;
