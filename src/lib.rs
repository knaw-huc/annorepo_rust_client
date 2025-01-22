use std::collections::HashMap;
use std::fmt;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug)]
pub enum Error {
    UrlNotFound,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::UrlNotFound => write!(f, "URL not found"),
        }
    }
}

#[derive(Debug)]
pub struct AnnoRepoClient {
    base_url: String,
    container: String,
    // api_key: String,
    client: reqwest::Client,
}

impl AnnoRepoClient {
    pub fn new(base_url: impl Into<String>, container: impl Into<String>) -> Result<Self, Error> {
        let annorepo_client = Self {
            base_url: base_url.into(),
            container: container.into(),
            // api_key: "".into(),
            client: reqwest::ClientBuilder::new()
                .user_agent(APP_USER_AGENT)
                .connection_verbose(true)
                .build()
                .unwrap(),
        };

        Ok(annorepo_client)
    }

    pub async fn get_about(&self) -> Result<HashMap<String, serde_json::Value>, reqwest::Error> {
        let url = format!("{}/about", self.base_url);

        Ok(self.client_get(url).await?)
    }

    pub async fn get_fields(&self) -> Result<HashMap<String, i32>, reqwest::Error> {
        let url = self.resolve_service("fields");

        Ok(self.client_get(url).await?)
    }

    pub async fn get_indexes(&self) -> Result<Vec<HashMap<String, String>>, reqwest::Error> {
        let url = self.resolve_service("indexes");

        Ok(self.client_get(url).await?)
    }

    fn resolve_service(&self, endpoint: &str) -> String {
        format!("{}/services/{}/{}", self.base_url, self.container, endpoint)
    }

    async fn client_get<T>(&self, url: String) -> Result<T, reqwest::Error>
    where
        T: serde::de::DeserializeOwned,
    {
        Ok(self.client.get(url).send().await?.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::AnnoRepoClient;

    #[test]
    fn client_is_setup_properly() {
        let base_url = "https://annorepo.example.com";
        let container = "example-container-1.0a";
        let client = AnnoRepoClient::new(base_url, container).unwrap();

        assert_eq!(client.base_url, base_url);
        assert_eq!(client.container, container);
    }
}
