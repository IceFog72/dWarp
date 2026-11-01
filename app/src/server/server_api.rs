use std::sync::Arc;

use warpui::{Entity, SingletonEntity};

/// Shared HTTP client for local tools and fork release downloads.
pub struct ServerApi {
    client: Arc<http_client::Client>,
}

impl ServerApi {
    pub fn http_client(&self) -> &http_client::Client {
        &self.client
    }
}

pub struct ServerApiProvider {
    server_api: Arc<ServerApi>,
}

impl ServerApiProvider {
    pub fn new() -> Self {
        let client = http_client::Client::new();
        Self {
            server_api: Arc::new(ServerApi {
                client: Arc::new(client),
            }),
        }
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self {
            server_api: Arc::new(ServerApi {
                client: Arc::new(http_client::Client::new_for_test()),
            }),
        }
    }

    pub fn get(&self) -> Arc<ServerApi> {
        self.server_api.clone()
    }

    pub fn get_http_client(&self) -> Arc<http_client::Client> {
        self.server_api.client.clone()
    }
}

impl Entity for ServerApiProvider {
    type Event = ();
}

impl SingletonEntity for ServerApiProvider {}
