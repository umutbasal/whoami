use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use json_to_table::json_to_table;
use std::collections::HashMap;
use std::net::IpAddr;
use std::{convert::Infallible, net::SocketAddr};
use sysinfo::{System, SystemExt};

#[tokio::main]
async fn main() {
    let addr = SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 8080);
    let server = Server::bind(&addr).serve(make_service_fn(move |conn: &AddrStream| {
        let addr = conn.remote_addr();
        async move {
            let addr = addr.clone();
            Ok::<_, Infallible>(service_fn(move |req| handle(req, addr.clone())))
        }
    }));

    println!("Listening on http://{}", addr);
    if let Err(e) = server.await {
        println!("server error: {}", e);
    }
}

async fn handle(req: Request<Body>, addr: SocketAddr) -> Result<Response<Body>, Infallible> {
    let mut sys = System::new_all();

    sys.refresh_all();

    let remote_ip = addr.ip().to_string();
    let headers = req.headers().clone();

    let mut headers_map = HashMap::new();
    for (name, value) in headers.iter() {
        headers_map.insert(name.to_string(), value.to_str().unwrap_or("").to_string());
    }

    let mut environment_map = HashMap::new();
    for (key, value) in std::env::vars() {
        environment_map.insert(key, value);
    }

    let json_data = serde_json::json!({
        "sysinfo": sys,
        "headers": headers_map,
        "remote_ip": remote_ip,
        "environment": environment_map
    });

    let mut output = String::new();

    for (key, value) in json_data.as_object().unwrap() {
        output = output + &format!("<h1>{}</h1><pre>{}</pre>", key, json_to_table(value));
    }

    if req.uri().query().map_or(false, |q| q.contains("json=true"))
        || req.headers().get("accept").map_or(false, |a| {
            a.to_str().unwrap_or("").contains("application/json")
        })
        || req
            .headers()
            .get("user-agent")
            .map_or(false, |a| a.to_str().unwrap_or("").contains("curl"))
    {
        return Ok(Response::new(Body::from(json_data.to_string())));
    }

    Ok(Response::new(Body::from(format!(
        "<html><head><title>Whoami</title></head><body>{}</body></html>",
        output
    ))))
}
