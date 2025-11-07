use std::sync::Arc;

use crate::options::TachyonOptions;

pub type TachyonHandler = Arc<Box<dyn Fn(TachyonOptions) + Send + Sync + 'static>>;

pub struct TachyonRouter {
    method: u8,
    handler: TachyonHandler,
}

impl TachyonRouter {
    pub fn new<F>(method: u8, handler: Arc<F>) -> Self
    where
        F: Fn(TachyonOptions) + Send + Sync + 'static,
    {
        let boxed_handler: Box<dyn Fn(TachyonOptions) + Send + Sync + 'static> =
            Box::new(move |opt| (*handler)(opt));

        Self {
            method,
            handler: Arc::new(boxed_handler),
        }
    }

    pub fn method(&self) -> u8 {
        self.method
    }

    pub fn handler(&self) -> &Arc<Box<dyn Fn(TachyonOptions) + Send + Sync + 'static>> {
        &self.handler
    }
}
