//! Wire types (docs/web/03 §DTO rule): camelCase, `ToSchema`, built from service types with
//! `From` impls. Service and core types are never serialized directly.

pub mod events;
pub mod system;
