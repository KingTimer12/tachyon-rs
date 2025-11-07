use crate::{Tachyon, methods::Method, options::TachyonOptions};
use std::sync::Arc;

pub trait HTTPCall {
  fn call<F>(
    &self,
    route: String,
    method: Method,
    callback: F,
  ) -> Result<(), Box<dyn std::error::Error>>
  where
    F: Fn(TachyonOptions) + Send + Sync + 'static;
}

impl HTTPCall for Tachyon {
  fn call<F>(
    &self,
    route: String,
    method: Method,
    callback: F,
  ) -> Result<(), Box<dyn std::error::Error>>
  where
    F: Fn(TachyonOptions) + Send + Sync + 'static,
  {
    let handler = Arc::new(callback);
    let route_key = format!("{}:{}", method.id(), route);
    let router = crate::router::TachyonRouter::new(method.id(), handler);
    self.routes().insert(route_key, router);
    Ok(())
  }
}