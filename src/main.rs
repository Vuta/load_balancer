use clap::Parser;
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use load_balancer::LoadBalancer;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = String::from("round_robin"))]
    algo: String,
    #[arg(short, long, default_value_t = 1)]
    count: u8,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let lb = Arc::new(LoadBalancer::new(args.algo, args.count));

    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let lb = lb.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, lb).await {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
