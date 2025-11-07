use std::{hash::BuildHasherDefault, sync::Arc};

use ahash::AHasher;
use bytes::Bytes;
use dashmap::DashMap;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response, StatusCode};

use crate::{
    cache::HotCache,
    methods::Method,
    options::TachyonOptions,
    router::TachyonRouter,
    utils::{self, empty, full},
};

type FastHasher = BuildHasherDefault<AHasher>;

const NOTFOUND: &[u8] = b"404 Not Found";

pub struct Tachyon {
    routes: Arc<DashMap<String, TachyonRouter, FastHasher>>,
    hot_cache: Arc<HotCache>,
}

impl Tachyon {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get<F>(&self, path: &str, callback: F)
    where
        F: Fn(TachyonOptions) + Send + Sync + 'static,
    {
        self.routes.insert(
            path.to_string(),
            TachyonRouter::new(Method::Get.id(), Arc::new(callback)),
        );
    }

    pub fn routes(&self) -> Arc<DashMap<String, TachyonRouter, FastHasher>> {
        self.routes.clone()
    }

    #[inline]
    async fn echo(
        routes: Arc<DashMap<String, TachyonRouter, FastHasher>>,
        hot_cache: Arc<HotCache>,
        req: Request<hyper::body::Incoming>,
    ) -> std::result::Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        let path = req.uri().path();
        let hyper_method = req.method();
        let method = Method::from(hyper_method);

        let mut route_key = String::with_capacity(path.len() + 3); // method_id + ':' + path
        route_key.push_str(itoa::Buffer::new().format(method.id()));
        route_key.push(':');
        route_key.push_str(path);

        let handler = if let Some(cached) = hot_cache.get(&route_key) {
            cached
        } else {
            match routes.get(&route_key) {
                Some(route_ref) => {
                    let handler = Arc::clone(route_ref.handler());
                    hot_cache.set(route_key.clone(), handler.clone());
                    handler
                }
                None => {
                    match routes
                        .iter()
                        .find(|entry| utils::route_matches(entry.key(), &route_key))
                    {
                        Some(entry) => {
                            let handler = Arc::clone(entry.value().handler());
                            // Cache parameterized routes too
                            hot_cache.set(route_key.clone(), handler.clone());
                            handler
                        }
                        None => {
                            // Early return for 404 - avoid further processing
                            return Ok(Response::builder()
                                .status(StatusCode::NOT_FOUND)
                                .body(full(NOTFOUND))
                                .unwrap());
                        }
                    }
                }
            }
        };

        Ok(Response::builder().body(empty()).unwrap())
    }
}

impl Default for Tachyon {
    fn default() -> Self {
        Self {
            routes: Arc::new(DashMap::with_hasher(FastHasher::default())),
            hot_cache: Arc::new(HotCache::new()),
        }
    }
}
