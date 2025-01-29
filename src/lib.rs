use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

const APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

const LOCATION_HEADER: &str = "location";

#[derive(Debug)]
pub enum Error {
    UrlNotFound,
    ReqError(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::UrlNotFound => write!(f, "URL not found"),
            Self::ReqError(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub struct AnnoRepoClient {
    base_url: String,
    container: String,
    // api_key: String,
    client: reqwest::Client,
}

impl AnnoRepoClient {
    pub fn new<S: Into<String>>(base_url: S, container: S) -> Result<Self, Error> {
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

    pub async fn get_about(&self) -> Result<Value, reqwest::Error> {
        let url = format!("{}/about", self.base_url);
        Ok(self.client.get(url).send().await?.json().await?)
    }

    pub async fn get_fields(&self) -> Result<Value, reqwest::Error> {
        let url = self.resolve_service("fields");

        Ok(self.client_get_json(&url).await?)
    }

    pub async fn get_indexes(&self) -> Result<Value, reqwest::Error> {
        let url = self.resolve_service("indexes");

        Ok(self.client.get(url).send().await?.json().await?)
    }

    pub async fn get_distinct_values(&self, field: &str) -> Result<Value, reqwest::Error> {
        let url = self.resolve_service_param("distinct-values", field);

        Ok(self.client.get(url).send().await?.json().await?)
    }

    pub async fn create_search(&self, query: HashMap<&str, &str>) -> Result<SearchInfo, Error> {
        let url = self.resolve_service("search");

        let res = self
            .client
            .post(url)
            .json(&query)
            .send()
            .await
            .map_err(|e| Error::ReqError(e))?;
        println!("res {:?}", res);

        if let Some(header) = res.headers().get(LOCATION_HEADER) {
            let hdr = header.to_str().expect("Header must be valid unicode");
            println!("got header: {}", hdr);
            let search_id = hdr.rsplit_once('/').unwrap().1;
            println!("search_id: {}", search_id);
            Ok(SearchInfo::new(
                self,
                search_id.to_string(),
                hdr.to_string(),
            ))?
        } else {
            Err(Error::UrlNotFound)
        }
    }

    fn resolve_service(&self, endpoint: &str) -> String {
        format!(
            "{base}/services/{container}/{endpoint}",
            base = self.base_url,
            container = self.container
        )
    }

    fn resolve_service_param(&self, endpoint: &str, param: &str) -> String {
        format!(
            "{base}/services/{container}/{endpoint}/{param}",
            base = self.base_url,
            container = self.container
        )
    }

    async fn client_get_json<T>(&self, url: &str) -> Result<T, reqwest::Error>
    where
        T: serde::de::DeserializeOwned,
    {
        Ok(self.client.get(url).send().await?.json().await?)
    }
}

#[derive(Debug)]
pub struct SearchInfo<'a> {
    client: &'a AnnoRepoClient,
    search_id: String,
    location: String,
}

impl<'a> SearchInfo<'a> {
    pub fn new(
        client: &'a AnnoRepoClient,
        search_id: String,
        location: String,
    ) -> Result<Self, Error> {
        let result = Self {
            client,
            search_id,
            location,
        };

        Ok(result)
    }

    pub fn search_id(&self) -> &String {
        &self.search_id
    }

    pub fn location(&self) -> &String {
        &self.location
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
