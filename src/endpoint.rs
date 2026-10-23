//! A Servarr service reached over the HTTP port, and what its answers amount to.

use std::sync::Arc;

use lemonfiber_ports::http::{Http, Method, Request, Response, JSON};
use lemonfiber_ports::service::Failure;

/// The header a Servarr service reads its API key from.
pub const API_KEY_HEADER: &str = "X-Api-Key";

const CONTENT_TYPE: &str = "Content-Type";

const DETAIL_LIMIT: usize = 200;

const MARKER: &str = "...";

const SAID_IN: [&str; 2] = ["message", "error"];

const SAID_PER_FAILURE: &str = "errorMessage";

const SAID_BETWEEN: &str = "; ";

/// A service reached over the HTTP port, and what to call it when something goes wrong.
pub struct Endpoint {
    http: Arc<dyn Http>,
    base: String,
    service: String,
}

impl Endpoint {
    /// An endpoint for `service`, reached at `base` over `http`.
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, service: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into(),
            service: service.into(),
        }
    }

    /// The service every failure is blamed on.
    #[must_use]
    pub fn service(&self) -> &str {
        &self.service
    }

    /// A URL for `path` under the base, the base's trailing slash dropped.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base.trim_end_matches('/'))
    }

    /// A request to `path` carrying `key`, its body declared as JSON where it has one.
    #[must_use]
    pub fn keyed_request(
        &self,
        method: Method,
        path: &str,
        key: &str,
        body: Option<String>,
    ) -> Request {
        let mut headers = vec![(API_KEY_HEADER.to_owned(), key.to_owned())];
        if body.is_some() {
            headers.push((CONTENT_TYPE.to_owned(), JSON.to_owned()));
        }
        Request {
            method,
            url: self.url(path),
            headers,
            body,
            pinned: None,
        }
    }

    /// Send a request, a transport failure read as the service not answering.
    ///
    /// # Errors
    ///
    /// [`Failure::Unavailable`] where nothing answered.
    pub async fn send(&self, request: &Request) -> Result<Response, Failure> {
        self.http
            .send(request)
            .await
            .map_err(|_| self.unavailable())
    }

    fn unavailable(&self) -> Failure {
        Failure::Unavailable {
            service: self.service.clone(),
        }
    }

    /// A non-success response: a refused credential, or an answer that cannot be used.
    #[must_use]
    pub fn refusal(&self, response: &Response) -> Failure {
        match response.status {
            401 | 403 => self.unauthorised(),
            _ => self.refused(&describe(response)),
        }
    }

    /// A rejected credential, blamed on the service.
    #[must_use]
    pub fn unauthorised(&self) -> Failure {
        Failure::Unauthorised {
            service: self.service.clone(),
        }
    }

    /// An answer that arrived and could not be used, for `detail`.
    #[must_use]
    pub fn refused(&self, detail: &str) -> Failure {
        Failure::Refused {
            service: self.service.clone(),
            detail: detail.to_owned(),
        }
    }

    /// Something the service does not do, for `detail`.
    #[must_use]
    pub fn unsupported(&self, detail: &str) -> Failure {
        Failure::Unsupported {
            service: self.service.clone(),
            detail: detail.to_owned(),
        }
    }

    /// Nothing where the response succeeded, the refusal its status amounts to otherwise.
    ///
    /// # Errors
    ///
    /// The refusal a non-success status amounts to.
    pub fn expect_success(&self, response: &Response) -> Result<(), Failure> {
        if response.is_success() {
            Ok(())
        } else {
            Err(self.refusal(response))
        }
    }

    /// A JSON body read as `T`, a body that does not parse named by `what`.
    ///
    /// # Errors
    ///
    /// The refusal a non-success status amounts to, or a body not in the shape of `T`.
    pub fn decode<T: serde::de::DeserializeOwned>(
        &self,
        response: &Response,
        what: &str,
    ) -> Result<T, Failure> {
        if !response.is_success() {
            return Err(self.refusal(response));
        }
        serde_json::from_str(&response.body).map_err(|err| {
            self.refused(&format!(
                "{what}: the answer is not in the shape expected at line {} column {}",
                err.line(),
                err.column()
            ))
        })
    }
}

/// A non-success response as a refusal's detail: the service's own words, withheld and
/// then shortened, or its status alone where it said none. From a JSON answer only the
/// words it says are taken — its `message` or `error`, or each validation failure's
/// `errorMessage` — never the values beside them.
#[must_use]
pub fn describe(response: &Response) -> String {
    let said = match serde_json::from_str::<serde_json::Value>(&response.body) {
        Ok(serde_json::Value::Array(failures)) => failures
            .iter()
            .filter_map(|failure| failure.get(SAID_PER_FAILURE)?.as_str())
            .collect::<Vec<_>>()
            .join(SAID_BETWEEN),
        Ok(answer) => SAID_IN
            .iter()
            .find_map(|field| answer.get(field).and_then(serde_json::Value::as_str))
            .unwrap_or_default()
            .to_owned(),
        Err(_) => response.body.clone(),
    };
    let body = lemonfiber_error::withheld::withheld_text(said.trim());
    let body = body.trim();
    if body.is_empty() {
        format!("HTTP {}", response.status)
    } else {
        format!("HTTP {}: {}", response.status, fitted(body, DETAIL_LIMIT))
    }
}

fn fitted(text: &str, width: usize) -> String {
    let counted = text.chars().count();
    if counted <= width {
        return text.to_owned();
    }
    let keep = width.saturating_sub(MARKER.len());
    let tail = keep / 2;
    let head = keep - tail;
    let front: String = text.chars().take(head).collect();
    let back: String = text.chars().skip(counted - tail).collect();
    format!("{front}{MARKER}{back}")
}

#[cfg(test)]
mod tests;
