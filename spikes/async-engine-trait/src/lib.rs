//! Compile spike for object-safe async engine traits.

use std::{future::Future, pin::Pin};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait Engine: Send + Sync {
    fn ping(&self) -> BoxFuture<'_, Result<(), String>>;
}

pub struct Fake;

impl Engine for Fake {
    fn ping(&self) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
}
