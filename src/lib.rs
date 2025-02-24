use http_body_util::Full;
use hyper::{Request, Response};
use hyper::body::{Incoming as IncomingBody, Bytes};
use hyper::service::Service;
use std::pin::Pin;
use rand::seq::IndexedRandom;

#[derive(Debug, Clone)]
pub struct LoadBalancer {
    backends: Vec<Backend>,
}

impl LoadBalancer {
    pub fn new() -> Self {
        let mut backends: Vec<Backend> = Vec::new();

        for i in 0..4 {
            backends.push(Backend { id: i, host: String::from("127.0.0.1") });
        }

        Self { backends }
    }

    fn forward(&self, req: Request<IncomingBody>) -> Response<Full<Bytes>> {
        println!("lb: {:?}, req: {:?}", self, req);

        let mut rng = rand::rng();
        let backend = self.backends.choose(&mut rng).unwrap();
        let msg = format!("hello from world {:?}", backend);

        Response::builder().body(Full::new(Bytes::from(msg))).unwrap()
    }
}

impl Service<Request<IncomingBody>> for LoadBalancer {
    type Response = Response<Full<Bytes>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<IncomingBody>) -> Self::Future {
        let res = Ok(self.forward(req));

        Box::pin(async { res })
    }
}

#[derive(Debug, Clone)]
struct Backend {
    id: usize,
    host: String,
}
