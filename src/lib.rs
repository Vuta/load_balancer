use http_body_util::Full;
use hyper::body::{Bytes, Incoming as IncomingBody};
use hyper::service::Service;
use hyper::{Request, Response};
use rand::seq::IndexedRandom;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
enum Algo {
    RoundRobin(Arc<AtomicUsize>),
    LeastConnection,
}

#[derive(Debug)]
pub struct LoadBalancer {
    algo: Algo,
    client: reqwest::Client,
    backends: Vec<Backend>,
}

impl LoadBalancer {
    pub fn new(algo: String, num_backends: u8) -> Self {
        let mut backends: Vec<Backend> = Vec::new();
        let client = reqwest::Client::new();

        for i in 0..num_backends {
            backends.push(Backend {
                host: format!("127.0.0.1:808{}", i),
                inflights: AtomicUsize::new(0),
            });
        }

        let algo = match algo.as_str() {
            "round_robin" => Algo::RoundRobin(Arc::new(AtomicUsize::new(0))),
            "least_connection" => Algo::LeastConnection,
            _ => panic!("unsupported algorithm"),
        };

        Self {
            algo,
            client,
            backends,
        }
    }

    fn select(&self) -> &Backend {
        match &self.algo {
            Algo::RoundRobin(rc) => {
                let len = &self.backends.len();
                let i = rc.load(Ordering::Relaxed);

                rc.store((rc.load(Ordering::Relaxed) + 1) % len, Ordering::Relaxed);

                &self.backends[i]
            }
            Algo::LeastConnection => {
                let min = &self
                    .backends
                    .iter()
                    .map(|be| be.inflights.load(Ordering::Relaxed))
                    .min()
                    .unwrap();

                let bes: &Vec<&Backend> = &self
                    .backends
                    .iter()
                    .filter(|be| be.inflights.load(Ordering::Relaxed) == *min)
                    .collect();

                let mut rng = rand::rng();
                let backend = bes.choose(&mut rng).unwrap();

                backend
            }
        }
    }
}

impl Service<Request<IncomingBody>> for LoadBalancer {
    type Response = Response<Full<Bytes>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<IncomingBody>) -> Self::Future {
        let backend = self.select();
        let uri = format!("http://{}{}", backend.host, req.uri());

        let request_builder = self
            .client
            .request(req.method().clone(), uri)
            .headers(req.headers().clone())
            .body(reqwest::Body::wrap(req));

        let _ = backend.inflights.fetch_add(1, Ordering::Relaxed);

        println!("{:?}", backend);

        let tsk = Box::pin(async {
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
        });

        let _ = backend.inflights.fetch_sub(1, Ordering::Relaxed);

        tsk
    }
}

#[derive(Debug)]
struct Backend {
    host: String,
    inflights: AtomicUsize,
}
