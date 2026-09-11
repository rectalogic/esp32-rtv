use atrium_xrpc::http::{Method, Request, Response, StatusCode};
use atrium_xrpc::{HttpClient, XrpcClient};
use esp_idf_svc::http::client::{Configuration, EspHttpConnection, Method as EspMethod};
use esp_idf_svc::io::Write;
use std::sync::Mutex;
use std::time::Duration;

struct SendEspHttpConnection(EspHttpConnection);

// SAFETY: EspHttpConnection is !Send only because it *can* hold a boxed
// `dyn Fn` event handler. Do not install an event handler while
// using this wrapper.
unsafe impl Send for SendEspHttpConnection {}

pub struct EspIdfXrpcClient {
    base_uri: String,
    connection: Mutex<SendEspHttpConnection>,
}

impl EspIdfXrpcClient {
    pub fn new(base_uri: impl Into<String>, config: Configuration) -> anyhow::Result<Self> {
        let connection = EspHttpConnection::new(&config)?;
        Ok(Self {
            base_uri: base_uri.into(),
            connection: Mutex::new(SendEspHttpConnection(connection)),
        })
    }

    pub fn new_default(base_uri: impl Into<String>) -> anyhow::Result<Self> {
        Self::new(
            base_uri,
            Configuration {
                buffer_size: Some(8192),
                buffer_size_tx: Some(2048),
                timeout: Some(Duration::from_secs(30)),
                crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
                ..Default::default()
            },
        )
    }
}

impl HttpClient for EspIdfXrpcClient {
    async fn send_http(
        &self,
        request: Request<Vec<u8>>,
    ) -> Result<Response<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        let (parts, body) = request.into_parts();
        let uri = parts.uri.to_string();

        let headers: Vec<(&str, &str)> = parts
            .headers
            .iter()
            .filter_map(|(name, value)| {
                let value = value.to_str().ok()?;
                Some((name.as_str(), value))
            })
            .collect();

        let method = match parts.method {
            Method::GET => EspMethod::Get,
            Method::POST => EspMethod::Post,
            Method::PUT => EspMethod::Put,
            Method::DELETE => EspMethod::Delete,
            Method::HEAD => EspMethod::Head,
            Method::OPTIONS => EspMethod::Options,
            Method::CONNECT => EspMethod::Connect,
            Method::PATCH => EspMethod::Patch,
            Method::TRACE => EspMethod::Trace,
            _ => return Err(anyhow::anyhow!("Invalid HTTP method: {}", parts.method).into()),
        };

        let mut guard = self.connection.lock().unwrap_or_else(|p| p.into_inner());
        let connection = &mut guard.0;
        let mut client = embedded_svc::http::client::Client::wrap(&mut *connection);

        let mut request = client
            .request(method, &uri, &headers)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        if !body.is_empty() {
            request
                .write_all(&body)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        }

        let mut response = request
            .submit()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        let mut response_body = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            match response.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => response_body.extend_from_slice(&buffer[..n]),
                Err(e) => return Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
            }
        }

        let status = response.status();
        let status_code = StatusCode::from_u16(status as u16)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        let mut response_builder = Response::builder().status(status_code);

        // esp-idf-svc does not provide an iterator for response headers.
        // We manually query headers commonly required by the AT Protocol (Bluesky).
        // https://github.com/esp-rs/embedded-svc/issues/19
        let important_headers = [
            "Content-Type",
            "Content-Length",
            "Link",
            "Location",
            "Set-Cookie",
            "Authorization",
            "WWW-Authenticate",
            "Retry-After",
            "ETag",
            "Last-Modified",
            "Cache-Control",
            "Content-Encoding",
            "Transfer-Encoding",
            "X-RateLimit-Limit",
            "X-RateLimit-Remaining",
            "X-RateLimit-Reset",
            "DPoP-Nonce",
        ];
        for name in important_headers.iter() {
            if let Some(value) = response.header(name) {
                response_builder = response_builder.header(*name, value);
            }
        }

        let http_response = response_builder
            .body(response_body)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        Ok(http_response)
    }
}

impl XrpcClient for EspIdfXrpcClient {
    fn base_uri(&self) -> String {
        self.base_uri.clone()
    }
}
