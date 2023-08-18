use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::net::IpAddr;
use std::{convert::Infallible, net::SocketAddr};
use sysinfo::{System, SystemExt};

#[tokio::main]
async fn main() {
    let addr = SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 8080);
    let server = Server::bind(&addr).serve(make_service_fn(|_| async {
        Ok::<_, Infallible>(service_fn(handle))
    }));

    println!("Listening on http://{}", addr);
    if let Err(e) = server.await {
        println!("server error: {}", e);
    }
}

async fn handle(_: Request<Body>) -> Result<Response<Body>, Infallible> {
    let mut sys = System::new_all();

    sys.refresh_all();

    Ok(Response::new(Body::from(
        serde_json::to_string(&sys).unwrap(),
    )))
}
