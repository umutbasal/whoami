use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use json_to_table::json_to_table;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{System, SystemExt};

struct Store {
    sys: Arc<Mutex<System>>,
    env: Arc<HashMap<String, String>>,
    ttl: Duration,
    last_refresh: Instant,
}

impl Store {
    fn new() -> Self {
        Store {
            sys: Arc::new(Mutex::new(System::new_all())),
            env: Arc::new(std::env::vars().collect()),
            ttl: Duration::from_secs(10),
            last_refresh: Instant::now(),
        }
    }

    fn refresh(&mut self) {
        self.sys.lock().unwrap().refresh_all();

        let mut new_env = HashMap::new();
        for (key, value) in std::env::vars() {
            new_env.insert(key, value);
        }
        self.env = Arc::new(new_env);

        self.last_refresh = Instant::now();
    }

    fn getsys(&mut self) -> Arc<Mutex<System>> {
        if self.last_refresh.elapsed() > self.ttl {
            self.refresh();
        }
        Arc::clone(&self.sys)
    }

    fn getenv(&mut self) -> Arc<HashMap<String, String>> {
        if self.last_refresh.elapsed() > self.ttl {
            self.refresh();
        }
        Arc::clone(&self.env)
    }
}

#[tokio::main]
async fn main() {
    let store = Arc::new(Mutex::new(Store::new()));

    let addr = SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 8080);
    let make_svc = make_service_fn(move |conn: &AddrStream| {
        let addr = conn.remote_addr();
        let store_clone = Arc::clone(&store);

        async move {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                handle(req, addr.clone(), Arc::clone(&store_clone))
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    println!("Listening on http://{}", addr);
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

fn value_limiter(flag: bool, val: String) -> String {
    if flag {
        return val;
    }
    // adds new line every 80 characters
    let mut output = String::new();
    let mut counter = 0;
    for c in val.chars() {
        if counter == 80 {
            output.push_str("\n");
            counter = 0;
        }
        output.push(c);
        counter += 1;
    }
    output
}

async fn handle(
    req: Request<Body>,
    addr: SocketAddr,
    store: Arc<Mutex<Store>>,
) -> Result<Response<Body>, hyper::Error> {
    let headers = req.headers().clone();
    let view_as_json = view_as_json(req);

    let mut headers_map = HashMap::new();
    for (name, value) in headers.iter() {
        headers_map.insert(
            name.to_string(),
            value_limiter(view_as_json, value.to_str().unwrap_or("").to_string()),
        );
    }

    let mut s = store.lock().unwrap();
    let sys = &*s.getsys();
    let env = &*s.getenv();

    let remote_ip = addr.ip().to_string();

    let json_data = serde_json::json!({
        "headers": headers_map,
        "environment": env,
        "sysinfo": sys,
        "remote_ip": remote_ip,
    });

    let mut output = String::new();

    for (key, value) in json_data.as_object().unwrap() {
        output = output
            + &format!(
                "\n<h1>{}</h1>\n<pre>\n{}\n</pre>",
                key,
                json_to_table(value)
            );
    }

    if view_as_json {
        return Ok(Response::new(Body::from(json_data.to_string())));
    }

    Ok(Response::new(Body::from(format!(
        "<html><head><title>Whoami</title></head><body>{}</body></html>",
        output
    ))))
}

fn view_as_json(req: Request<Body>) -> bool {
    req.uri().query().map_or(false, |q| q.contains("j"))
        || req.uri().path().contains("j")
        || req.headers().get("accept").map_or(false, |a| {
            a.to_str().unwrap_or("").contains("application/json")
        })
        || (req
            .headers()
            .get("user-agent")
            .map_or(false, |a| a.to_str().unwrap_or("").contains("curl"))
            && !(req.uri().query().map_or(false, |q| q.contains("h"))
                || req.uri().path().contains("h")))
}

#[cfg(test)]
mod tests {
    use sysinfo::SystemExt;

    #[test]
    fn value_limit_fail_not_json() {
        let mut input = "".to_string();
        for _ in 0..100 {
            input.push_str("a");
        }
        let got = super::value_limiter(false, input.to_string());
        let want = input.to_string();
        assert_ne!(got, want);
    }

    #[test]
    fn value_limit_pass_not_json() {
        let mut input = "".to_string();
        for _ in 0..500 {
            input.push_str("a");
        }
        let got = super::value_limiter(false, input.to_string());

        for line in got.lines() {
            assert!(line.len() <= 80);
        }

        assert!(got.lines().count() == 7)
    }

    #[test]
    fn value_limit_pass_json() {
        let mut input = "".to_string();
        for _ in 0..500 {
            input.push_str("a");
        }
        let got = super::value_limiter(true, input.to_string());

        assert!(got.lines().count() == 1)
    }

    #[test]
    fn view_as_json_pass_json_param() {
        let req = hyper::Request::builder()
            .uri("http://localhost:8080/?j")
            .body(hyper::Body::empty())
            .unwrap();
        let got = super::view_as_json(req);
        assert!(got);

        let req2 = hyper::Request::builder()
            .uri("http://localhost:8080/j")
            .body(hyper::Body::empty())
            .unwrap();
        let got2 = super::view_as_json(req2);
        assert!(got2);
    }

    #[test]
    fn view_as_json_pass_html_param() {
        let req = hyper::Request::builder()
            .uri("http://localhost:8080/h")
            .body(hyper::Body::empty())
            .unwrap();
        let got = super::view_as_json(req);
        assert!(!got);

        let req2 = hyper::Request::builder()
            .uri("http://localhost:8080/?h")
            .body(hyper::Body::empty())
            .unwrap();

        let got2 = super::view_as_json(req2);
        assert!(!got2);
    }

    #[tokio::test]
    async fn os_env_test() {
        std::env::set_var("TEST_ENV_VAR", "test_value");
        // addr
        let addr = super::SocketAddr::new(super::IpAddr::from([0, 0, 0, 0]), 8080);
        let store = super::Arc::new(super::Mutex::new(super::Store::new()));
        let req = hyper::Request::builder()
            .uri("http://localhost:8080/h")
            .body(hyper::Body::empty())
            .unwrap();
        let got = super::handle(req, addr, store).await.unwrap();
        let body = hyper::body::to_bytes(got.into_body()).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("TEST_ENV_VAR"));
        assert!(body.contains("test_value"));
    }

    #[tokio::test]
    async fn sysinfo_shown() {
        // addr
        let addr = super::SocketAddr::new(super::IpAddr::from([0, 0, 0, 0]), 8080);
        let store = super::Arc::new(super::Mutex::new(super::Store::new()));
        let req = hyper::Request::builder()
            .uri("http://localhost:8080/h")
            .body(hyper::Body::empty())
            .unwrap();
        let got = super::handle(req, addr, store).await.unwrap();
        let body = hyper::body::to_bytes(got.into_body()).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();

        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
        let want = sys.host_name().unwrap();

        assert!(body.contains(&want));
    }

    #[tokio::test]
    async fn headers_shown() {
        // addr
        let addr = super::SocketAddr::new(super::IpAddr::from([0, 0, 0, 0]), 8080);
        let store = super::Arc::new(super::Mutex::new(super::Store::new()));
        let req = hyper::Request::builder()
            .uri("http://localhost:8080/h")
            .header("test_header", "test_value")
            .body(hyper::Body::empty())
            .unwrap();
        let got = super::handle(req, addr, store).await.unwrap();
        let body = hyper::body::to_bytes(got.into_body()).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert!(body.contains("test_header"));
        assert!(body.contains("test_value"));
    }

    #[tokio::test]
    async fn remote_ip_shown() {
        // addr
        let addr = super::SocketAddr::new(super::IpAddr::from([0, 0, 0, 0]), 8080);
        let store = super::Arc::new(super::Mutex::new(super::Store::new()));
        let req = hyper::Request::builder()
            .uri("http://localhost:8080/h")
            .body(hyper::Body::empty())
            .unwrap();
        let got = super::handle(req, addr, store).await.unwrap();
        let body = hyper::body::to_bytes(got.into_body()).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert!(body.contains("remote_ip"));
        assert!(body.contains("0.0.0.0"));
    }
}
