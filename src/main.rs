use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use json_to_table::json_to_table;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::process::Command;
use std::str::FromStr;
use std::{convert::Infallible, net::SocketAddr};
use sysinfo::{System, SystemExt};

#[tokio::main]
async fn main() {
    let addr = SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 8080);
    let server = Server::bind(&addr).serve(make_service_fn(move |conn: &AddrStream| {
        let addr = conn.remote_addr();
        async move { Ok::<_, Infallible>(service_fn(move |req| handle(req, addr))) }
    }));

    println!("Listening on http://{}", addr);
    if let Err(e) = server.await {
        println!("server error: {}", e);
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
            output.push('\n');
            counter = 0;
        }
        output.push(c);
        counter += 1;
    }
    output
}

async fn handle(req: Request<Body>, addr: SocketAddr) -> Result<Response<Body>, Infallible> {
    let headers = req.headers().clone();
    let view_as_json = view_as_json(req);

    let mut headers_map = HashMap::new();
    for (name, value) in headers.iter() {
        headers_map.insert(
            name.to_string(),
            value_limiter(view_as_json, value.to_str().unwrap_or("").to_string()),
        );
    }

    let mut environment_map = HashMap::new();
    for (key, value) in std::env::vars() {
        environment_map.insert(key, value_limiter(view_as_json, value));
    }

    let mut sys = System::new_all();
    sys.refresh_all();

    let remote_ip = addr.ip().to_string();

    let (ipv4, ipv6) = public_ips().await;

    let public_map = serde_json::json!({
        "ipv4": ipv4.to_string(),
        "ipv6": ipv6.to_string(),
    });

    let mut am_i_isolated = match am_i_isolated() {
        Ok(isolation_posture) => isolation_posture.data,
        Err(_) => HashMap::new(),
    };

    if am_i_isolated.is_empty() {
        am_i_isolated.insert(
            "No isolation posture data".to_string(),
            vec!["".to_string()],
        );
    }

    let json_data = serde_json::json!({
        "1_headers": headers_map,
        "2_environment": environment_map,
        "3_remote_ip": remote_ip,
        "4_public_ips": public_map,
        "5_am_i_isolated": am_i_isolated,
        "6_sysinfo": sys,
    });

    let mut output = String::new();

    for (key, value) in json_data.as_object().unwrap() {
        let key = clean_prefixes(key);
        output = output
            + &format!(
                "\n<h1>{}</h1>\n<pre>\n{}\n</pre>",
                key,
                json_to_table(value)
            );
    }

    if view_as_json {
        let mut cleaned_json = serde_json::Map::new();
        for (key, value) in json_data.as_object().unwrap() {
            let cleaned_key = clean_prefixes(key).to_string();
            cleaned_json.insert(cleaned_key, value.clone());
        }
        let mut output = serde_json::Value::Object(cleaned_json).to_string();
        output = replace_emojis(&output);
        return Ok(Response::new(Body::from(output.to_string())));
    }

    let mut response = Response::new(Body::from(format!(
        "<html><head><title>Whoami</title></head><body>{}</body></html>",
        output
    )));
    response.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("text/html; charset=utf-8"),
    );
    Ok(response)
}

fn clean_prefixes(key: &String) -> &str {
    let key = key.trim_start_matches(|c: char| c.is_ascii_digit() || c == '_');
    key
}

pub fn mock_isolated() -> String {
    let output = "High Priority\n a\n b\n c\nLow Priority\n d\n e\n f\n";
    output.to_string()
}

fn parse_isolation_output(output: &str) -> HashMap<String, Vec<String>> {
    let output = output
        .lines()
        .filter(|line| line.starts_with(' ') || line.contains("Priority"))
        .collect::<Vec<&str>>()
        .join("\n");

    let mut data = HashMap::new();
    let mut key = String::new();
    for line in output.lines() {
        if line.starts_with(' ') {
            data.entry(key.clone())
                .or_insert_with(Vec::new)
                .push(line.trim().to_string());
        } else {
            key = line.trim().to_string();
        }
    }

    data
}

pub struct IsolationPosture {
    data: HashMap<String, Vec<String>>,
}

pub fn am_i_isolated() -> Result<IsolationPosture, String> {
    let cmd = Command::new("/app/am-i-isolated").output();

    let output = match cmd {
        Ok(output) => match String::from_utf8(output.stdout) {
            Ok(output) => output,
            Err(_) => return Err("Failed to parse command output".to_string()),
        },
        Err(_) => return Err("Failed to execute process".to_string()),
    };

    let data = parse_isolation_output(&output);

    Ok(IsolationPosture { data })
}

pub async fn public_ips() -> (Ipv4Addr, Ipv6Addr) {
    let cmd = Command::new("sh")
        .arg("-c")
        .arg("curl -s https://ipv4.icanhazip.com")
        .output()
        .expect("failed to execute process");

    let ipv4 = String::from_utf8(cmd.stdout)
        .unwrap_or("".to_string())
        .trim()
        .to_string();
    let ipv4 = Ipv4Addr::from_str(&ipv4).unwrap();

    //dig +short AAAA ipv6.icanhazip.com
    let cmd = Command::new("sh")
        .arg("-c")
        .arg("dig +short AAAA ipv6.icanhazip.com | head -n 1")
        .output()
        .expect("failed to execute process");

    let aaaa = String::from_utf8(cmd.stdout)
        .unwrap_or("".to_string())
        .trim()
        .to_string();
    let ipv6 = Ipv6Addr::from_str(&aaaa).unwrap_or(Ipv6Addr::UNSPECIFIED);

    let cmd = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "curl -k -6 -s https://[{}] --header 'Host: ipv6.icanhazip.com'",
            ipv6
        ))
        .output()
        .expect("failed to execute process");

    let ipv6 = String::from_utf8(cmd.stdout)
        .unwrap_or("".to_string())
        .trim()
        .to_string();
    let ipv6 = Ipv6Addr::from_str(&ipv6).unwrap_or(Ipv6Addr::UNSPECIFIED);

    (ipv4, ipv6)
}

fn view_as_json(req: Request<Body>) -> bool {
    req.uri().query().map_or(false, |q| q.contains('j'))
        || req.uri().path().contains('j')
        || req.headers().get("accept").map_or(false, |a| {
            a.to_str().unwrap_or("").contains("application/json")
        })
        || (req
            .headers()
            .get("user-agent")
            .map_or(false, |a| a.to_str().unwrap_or("").contains("curl"))
            && !(req.uri().query().map_or(false, |q| q.contains('h'))
                || req.uri().path().contains('h')))
}

fn replace_emojis(text: &str) -> String {
    let re = regex::Regex::new(r"🔥|😬|🤔|🔴|🟡|🟢").unwrap();

    re.replace_all(text, |caps: &regex::Captures| {
        match caps.get(0).unwrap().as_str() {
            "🔥" => "[HIGH]",
            "😬" => "[MEDIUM]",
            "🤔" => "[Low]",
            "🔴" => "[FAILED]",
            "🟡" => "[LOW]",
            "🟢" => "[PASSED]",
            _ => "",
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sysinfo::SystemExt;

    #[test]
    fn value_limit_fail_not_json() {
        let mut input = "".to_string();
        for _ in 0..100 {
            input.push('a');
        }
        let got = super::value_limiter(false, input.to_string());
        let want = input.to_string();
        assert_ne!(got, want);
    }

    #[test]
    fn value_limit_pass_not_json() {
        let mut input = "".to_string();
        for _ in 0..500 {
            input.push('a');
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
            input.push('a');
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
        // addr
        let addr = super::SocketAddr::new(super::IpAddr::from([0, 0, 0, 0]), 8080);
        // set an env var
        std::env::set_var("TEST_ENV_VAR", "test_value");
        let req = hyper::Request::builder()
            .uri("http://localhost:8080/h")
            .body(hyper::Body::empty())
            .unwrap();
        let got = super::handle(req, addr).await.unwrap();
        let body = hyper::body::to_bytes(got.into_body()).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("TEST_ENV_VAR"));
        assert!(body.contains("test_value"));
    }

    #[tokio::test]
    async fn sysinfo_shown() {
        // addr
        let addr = super::SocketAddr::new(super::IpAddr::from([0, 0, 0, 0]), 8080);
        let req = hyper::Request::builder()
            .uri("http://localhost:8080/h")
            .body(hyper::Body::empty())
            .unwrap();
        let got = super::handle(req, addr).await.unwrap();
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
        let req = hyper::Request::builder()
            .uri("http://localhost:8080/h")
            .header("test_header", "test_value")
            .body(hyper::Body::empty())
            .unwrap();
        let got = super::handle(req, addr).await.unwrap();
        let body = hyper::body::to_bytes(got.into_body()).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert!(body.contains("test_header"));
        assert!(body.contains("test_value"));
    }

    #[tokio::test]
    async fn remote_ip_shown() {
        // addr
        let addr = super::SocketAddr::new(super::IpAddr::from([0, 0, 0, 0]), 8080);
        let req = hyper::Request::builder()
            .uri("http://localhost:8080/h")
            .body(hyper::Body::empty())
            .unwrap();
        let got = super::handle(req, addr).await.unwrap();
        let body = hyper::body::to_bytes(got.into_body()).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert!(body.contains("remote_ip"));
        assert!(body.contains("0.0.0.0"));
    }

    #[test]
    fn test_parse_isolation_output() {
        let output = &mock_isolated();
        let parsed = parse_isolation_output(output);

        assert_eq!(
            parsed.get("High Priority").unwrap(),
            &vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(
            parsed.get("Low Priority").unwrap(),
            &vec!["d".to_string(), "e".to_string(), "f".to_string()]
        );
    }
}
