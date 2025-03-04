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
            let host = format!("127.0.0.1:808{}", i);
            let res = reqwest::blocking::get(format!("http://{}/health", host));

            let healthy = match res {
                Ok(res) => {
                    match res.status() {
                        reqwest::StatusCode::OK => true,
                        _ => false,
                    }
                }
                Err(_) => false,
            };

            backends.push(Backend {
                host: host,
                inflights: AtomicUsize::new(0),
                healthy: healthy,
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

    fn select(&self) -> Option<&Backend> {
        let backends: &Vec<&Backend> = &self.backends.iter().filter(|be| be.healthy == true).collect();

        if backends.len() == 0 {
            return None;
        }

        match &self.algo {
            Algo::RoundRobin(rc) => {
                let len = backends.len();
                let i = rc.load(Ordering::Relaxed) % len;

                rc.store((rc.load(Ordering::Relaxed) + 1) % len, Ordering::Relaxed);

                Some(&backends[i])
            }
            Algo::LeastConnection => {
                let min = backends
                    .into_iter()
                    .map(|be| be.inflights.load(Ordering::Relaxed))
                    .min()
                    .unwrap();

                let bes: &Vec<&Backend> = &backends
                    .into_iter()
                    .filter(|be| (*be).inflights.load(Ordering::Relaxed) == min)
                    .map(|be| *be)
                    .collect();

                let mut rng = rand::rng();
                let backend = bes.choose(&mut rng).unwrap();

                Some(backend)
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

        match backend {
            None => panic!("WTF"),
            Some(backend) => {
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
    }
}

#[derive(Debug)]
struct Backend {
    host: String,
    inflights: AtomicUsize,
    healthy: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        let lb = LoadBalancer::new(String::from("round_robin"), 1);

        assert_eq!(lb.backends.len(), 1);
        assert!(matches!(lb.algo, Algo::RoundRobin(_)));
    }

    #[test]
    fn select_none() {
        let lb = LoadBalancer::new(String::from("least_connection"), 2);
        let backend = lb.select();
        assert!(matches!(backend, None));
    }

    #[test]
    fn select_least_connection() {
        let mut lb = LoadBalancer::new(String::from("least_connection"), 2);

        {
            let backend = &mut lb.backends[0];
            backend.healthy = true;

            let backend = &mut lb.backends[1];
            backend.inflights = 1.into();
            backend.healthy = true;
            assert_eq!(backend.host, "127.0.0.1:8081");
        }

        let backend = lb.select().unwrap();
        assert_eq!(backend.inflights.load(Ordering::Relaxed), 0);
        assert_eq!(backend.host, "127.0.0.1:8080");
    }

    #[test]
    fn select_round_robin() {
        let mut lb = LoadBalancer::new(String::from("round_robin"), 2);
        for backend in &mut lb.backends {
            backend.healthy = true;
        }

        assert_eq!(lb.select().unwrap().host, "127.0.0.1:8080");
        assert_eq!(lb.select().unwrap().host, "127.0.0.1:8081");
        assert_eq!(lb.select().unwrap().host, "127.0.0.1:8080");
    }
}
