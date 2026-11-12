use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tiny_http::{Header, Response, Server};

pub struct ManifestServer {
    port: u16,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl ManifestServer {
    pub fn start(manifest_name: &'static str, manifest_content: String) -> Result<Self, String> {
        let server = Server::http("127.0.0.1:0")
            .map_err(|e| format!("Failed to bind local manifest server: {e}"))?;
        let port = server
            .server_addr()
            .to_ip()
            .map(|a| a.port())
            .ok_or_else(|| "Failed to get local server port".to_string())?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);

        let handle = thread::spawn(move || {
            while !stop_flag.load(Ordering::Relaxed) {
                match server.recv_timeout(Duration::from_millis(250)) {
                    Ok(Some(req)) => {
                        let url_path = req.url().split('?').next().unwrap_or("/");
                        let trimmed = url_path.trim_start_matches('/');

                        if trimmed == manifest_name
                            || trimmed == format!("{manifest_name}.manifest")
                            || trimmed == format!("client/{manifest_name}")
                        {
                            let resp = Response::from_string(manifest_content.clone())
                                .with_status_code(200);
                            let _ = req.respond(resp);
                        } else {
                            let file_part = trimmed.strip_prefix("client/").unwrap_or(trimmed);
                            let target =
                                format!("https://client-fastly.steamstatic.com/client/{file_part}");
                            if let Ok(header) = Header::from_bytes(&b"Location"[..], target.as_bytes()) {
                                let resp = Response::empty(302).with_header(header);
                                let _ = req.respond(resp);
                            } else {
                                let _ = req.respond(Response::empty(404));
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            port,
            stop,
            handle: Some(handle),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for ManifestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

