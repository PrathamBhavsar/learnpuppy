fn main() {
    let port = match e2e_port(std::env::args().skip(1), std::env::var("LP_E2E_PORT").ok()) {
        Ok(port) => port,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    if let Some(port) = port {
        serve_e2e(port);
    } else {
        dioxus::LaunchBuilder::desktop()
            .with_cfg(
                dioxus::desktop::Config::new().with_window(
                    dioxus::desktop::WindowBuilder::new()
                        .with_title("LearnPuppy")
                        .with_inner_size(dioxus::desktop::LogicalSize::new(1280.0, 820.0)),
                ),
            )
            .launch(lp_app::app);
    }
}

fn serve_e2e(port: u16) {
    use dioxus::liveview::LiveviewRouter;

    println!("LearnPuppy E2E: http://127.0.0.1:{port}");
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("create E2E runtime")
        .block_on(e2e_router_for(port).start(([127, 0, 0, 1], port)));
}

/// Rejects non-loopback Host (DNS rebinding) and cross-origin requests (WebSocket hijack by a web page).
fn loopback_only(port: u16, host: Option<&str>, origin: Option<&str>) -> bool {
    let local = |h: &str| h == format!("127.0.0.1:{port}") || h == format!("localhost:{port}");
    host.is_some_and(local) && origin.is_none_or(|o| o.strip_prefix("http://").is_some_and(local))
}

fn e2e_router_for(port: u16) -> dioxus::server::axum::Router {
    use dioxus::server::axum::{
        extract::Request,
        http::StatusCode,
        middleware::{from_fn, Next},
        response::Response,
    };
    e2e_router().layer(from_fn(move |req: Request, next: Next| async move {
        let ok = {
            let header = |n| req.headers().get(n).and_then(|v| v.to_str().ok());
            loopback_only(port, header("host"), header("origin"))
        };
        if ok {
            Ok(next.run(req).await)
        } else {
            Err::<Response, _>(StatusCode::FORBIDDEN)
        }
    }))
}

fn e2e_router() -> dioxus::server::axum::Router {
    use dioxus::liveview::LiveviewRouter;
    use dioxus::server::axum::{response::Html, routing::get, Router};

    Router::new().with_app("/", lp_app::app).route(
        "/",
        get(|| async {
            Html(format!(
                "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>LearnPuppy</title></head><body><div id=\"main\"></div>{}</body></html>",
                dioxus::liveview::interpreter_glue("/ws")
            ))
        }),
    )
}

fn e2e_port(
    args: impl IntoIterator<Item = String>,
    env_port: Option<String>,
) -> Result<Option<u16>, String> {
    let mut selected = env_port;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--e2e-port" {
            selected = Some(args.next().ok_or("--e2e-port requires a port")?);
        } else if let Some(port) = arg.strip_prefix("--e2e-port=") {
            selected = Some(port.to_owned());
        } else {
            return Err(format!("Unknown argument: {arg}"));
        }
    }
    selected
        .map(|port| {
            port.parse::<u16>()
                .ok()
                .filter(|port| *port != 0)
                .ok_or_else(|| "E2E port must be an integer between 1 and 65535".to_string())
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::{e2e_port, e2e_router, loopback_only};

    #[tokio::test]
    async fn e2e_root_serves_the_liveview_mount_instead_of_a_wildcard_404() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let router = e2e_router();
        let server = tokio::spawn(async move {
            dioxus::server::axum::serve(listener, router).await.unwrap();
        });
        let response = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
            client
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).await.unwrap();
            response
        })
        .await;
        server.abort();
        let response = response.unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("<title>LearnPuppy</title>"));
        assert!(response.contains("id=\"main\""));
        assert!(response.contains("/ws"));
    }

    #[test]
    fn cli_port_overrides_environment() {
        assert_eq!(
            e2e_port(["--e2e-port".into(), "9000".into()], Some("8000".into())),
            Ok(Some(9000))
        );
        assert_eq!(e2e_port(["--e2e-port=9001".into()], None), Ok(Some(9001)));
        assert_eq!(e2e_port([], Some("8000".into())), Ok(Some(8000)));
        assert_eq!(e2e_port([], None), Ok(None));
    }

    #[test]
    fn rejects_invalid_ports() {
        for args in [
            vec!["--e2e-port"],
            vec!["--e2e-port=0"],
            vec!["--e2e-port=65536"],
            vec!["--e2e-port=abc"],
        ] {
            assert!(e2e_port(args.into_iter().map(str::to_owned), None).is_err());
        }
    }

    #[test]
    fn loopback_gate_blocks_foreign_host_and_origin() {
        assert!(loopback_only(9, Some("127.0.0.1:9"), None));
        assert!(loopback_only(
            9,
            Some("localhost:9"),
            Some("http://localhost:9")
        ));
        assert!(!loopback_only(9, Some("evil.com:9"), None));
        assert!(!loopback_only(
            9,
            Some("127.0.0.1:9"),
            Some("http://evil.com")
        ));
        assert!(!loopback_only(9, None, None));
    }
}
