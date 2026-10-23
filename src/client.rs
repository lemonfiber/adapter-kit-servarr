//! The Servarr client: one service's versioned API, asked with the key lemonfiber wrote
//! for the adapter in front of it.

use std::sync::Arc;

use lemonfiber_ports::http::{Http, Method, Request, Response};
use lemonfiber_ports::service::{Failure, Identity};
use serde::Deserialize;

use crate::credential::Holding;
use crate::endpoint::Endpoint;

const NOT_SERVED: u16 = 404;

const PING: &str = "/ping";

const STATUS: &str = "/system/status";

/// A client for one Servarr service, presenting the key `held` supplies on every call.
pub struct Servarr {
    endpoint: Endpoint,
    held: Arc<dyn Holding>,
    version: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    #[serde(default)]
    instance_name: String,
    #[serde(default)]
    app_name: String,
    #[serde(default)]
    version: String,
}

impl Servarr {
    /// The service named `service`, reached at `base` over `http`, speaking its API
    /// `version` with the key `held` supplies.
    pub fn new(
        http: Arc<dyn Http>,
        base: impl Into<String>,
        held: Arc<dyn Holding>,
        service: impl Into<String>,
        version: u32,
    ) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
            held,
            version,
        }
    }

    /// The endpoint every call is sent through.
    #[must_use]
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Whether the service answers what it answers without a key.
    pub async fn answers(&self) -> bool {
        let ping = Request {
            method: Method::Get,
            url: self.endpoint.url(PING),
            headers: Vec::new(),
            body: None,
            pinned: None,
        };
        self.endpoint
            .send(&ping)
            .await
            .is_ok_and(|response| response.is_success())
    }

    fn key(&self) -> Result<String, Failure> {
        self.held.key().map_err(|why| {
            self.endpoint.refused(&format!(
                "this adapter holds no key for {}: {why}",
                self.endpoint.service()
            ))
        })
    }

    /// `path` under the versioned API asked with `method` and `body`, a versioned prefix
    /// the service does not serve read as an API version it does not speak.
    ///
    /// # Errors
    ///
    /// No key held, nothing answering, or the versioned prefix not served.
    pub async fn ask(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<Response, Failure> {
        let key = self.key()?;
        let request = self.endpoint.keyed_request(
            method,
            &format!("/api/v{}{path}", self.version),
            &key,
            body,
        );
        let response = self.endpoint.send(&request).await?;
        if response.status == NOT_SERVED {
            return Err(self.endpoint.unsupported(&format!(
                "there is no /api/v{}{path}; it may be a release this adapter does not speak",
                self.version
            )));
        }
        Ok(response)
    }

    /// `path` under the versioned API asked with `method` and `body`, and taken.
    ///
    /// # Errors
    ///
    /// What [`Servarr::ask`] refuses, or the refusal a non-success status amounts to.
    pub async fn done(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<(), Failure> {
        let response = self.ask(method, path, body).await?;
        self.endpoint.expect_success(&response)
    }

    /// What `path` under the versioned API holds, read as `T`; `what` names it where it
    /// cannot be read.
    ///
    /// # Errors
    ///
    /// What [`Servarr::ask`] refuses, or an answer not in the shape of `T`.
    pub async fn read<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        what: &str,
    ) -> Result<T, Failure> {
        let response = self.ask(Method::Get, path, None).await?;
        self.endpoint
            .decode(&response, &format!("{what} could not be read"))
    }

    /// Who the service is and which release it runs.
    ///
    /// # Errors
    ///
    /// What [`Servarr::read`] refuses, or a status naming neither the service nor its
    /// release.
    pub async fn identity(&self) -> Result<Identity, Failure> {
        let status: Status = self.read(STATUS, "the status").await?;
        let name = if status.instance_name.is_empty() {
            status.app_name
        } else {
            status.instance_name
        };
        if name.is_empty() || status.version.is_empty() {
            return Err(self
                .endpoint
                .refused("the service named neither itself nor its version"));
        }
        Ok(Identity {
            name,
            version: status.version,
        })
    }
}

#[cfg(test)]
mod tests;
