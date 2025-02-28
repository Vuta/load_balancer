use bytes::Bytes;
use hyper::{Request, Response};
use std::convert::Infallible;
use std::net::SocketAddr;

use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::time::{sleep, Instant, Duration};

use clap::Parser;
use rand::Rng;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    port: u16,
}

async fn hello(
    req: Request<impl hyper::body::Body + std::fmt::Debug>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let now = Instant::now();
    let x = {
        let mut rng = rand::rng();
        rng.random_range(0..=2000)
    };

    sleep(Duration::from_millis(x)).await;

    println!("{:?} took {} ms", req, now.elapsed().as_millis());

    Ok(Response::new(Full::new(Bytes::from("Hello World!"))))
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();
    let addr: SocketAddr = ([127, 0, 0, 1], args.port).into();

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);
    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(hello))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
