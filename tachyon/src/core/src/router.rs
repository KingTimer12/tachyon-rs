use std::sync::Arc;

use crate::options::{ResponseData, TachyonOptions};

pub type TachyonHandler = Arc<dyn Fn(TachyonOptions) -> ResponseData + Send + Sync + 'static>;

pub struct TachyonRouter {
    method: u8,
    handler: TachyonHandler,
}

impl TachyonRouter {
    pub fn new<F>(method: u8, handler: Arc<F>) -> Self
    where
        F: Fn(TachyonOptions) -> ResponseData + Send + Sync + 'static,
    {
        Self {
            method,
            handler: handler as TachyonHandler,
        }
    }

    pub fn method(&self) -> u8 {
        self.method
    }

    pub fn handler(&self) -> &TachyonHandler {
        &self.handler
    }
}
