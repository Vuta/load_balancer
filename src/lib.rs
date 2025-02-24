use http_body_util::Full;
use hyper::body::{Bytes, Incoming as IncomingBody};
use hyper::service::Service;
use hyper::{Request, Response};
use rand::seq::IndexedRandom;
use std::pin::Pin;

const NUM_OF_BACKENDS: usize = 1;

#[derive(Debug, Clone)]
pub struct LoadBalancer {
    client: reqwest::Client,
    backends: Vec<Backend>,
}

impl LoadBalancer {
    pub fn new() -> Self {
        let mut backends: Vec<Backend> = Vec::new();
        let client = reqwest::Client::new();

        for i in 0..NUM_OF_BACKENDS {
            backends.push(Backend {
                host: format!("127.0.0.1:808{}", i),
            });
        }

        Self { client, backends }
    }
}

impl Service<Request<IncomingBody>> for LoadBalancer {
    type Response = Response<Full<Bytes>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<IncomingBody>) -> Self::Future {
        let mut rng = rand::rng();
        let backend = self.backends.choose(&mut rng).unwrap();
        let uri = format!("http://{}{}", backend.host, req.uri());
        let request_builder = self
            .client
            .request(req.method().clone(), uri)
            .headers(req.headers().clone())
            .body(reqwest::Body::wrap(req));

        Box::pin(async {
            let backend_res = request_builder.send().await.unwrap();

            let mut builder = Response::builder();
            for (k, v) in backend_res.headers().iter() {
                builder = builder.header(k, v);
            }

            let response = builder
                .status(backend_res.status())
                .body(Full::new(backend_res.bytes().await.unwrap()))
                .unwrap();

            Ok(response)
        })
    }
}

#[derive(Debug, Clone)]
struct Backend {
    host: String,
}
