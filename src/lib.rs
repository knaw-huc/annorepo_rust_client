use serde_json::Value;
use serde_json::Value::Array;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt;

const APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

const LOCATION_HEADER: &str = "location";

#[derive(Debug)]
pub enum Error {
    UrlNotFound,
    MalformedAnnotationPage(Value),
    ReqError(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::UrlNotFound => write!(f, "URL not found"),
            Self::MalformedAnnotationPage(json) => {
                write!(f, "Malformed annotation page: {:?}", json)
            }
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

        Ok(self.client_get_json(&url).await?)
    }

    pub async fn get_distinct_values(&self, field: &str) -> Result<Value, reqwest::Error> {
        let url = self.resolve_service_param("distinct-values", field);

        Ok(self.client_get_json(&url).await?)
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

        if let Some(header) = res.headers().get(LOCATION_HEADER) {
            let location = header.to_str().expect("Header must be valid unicode");
            let search_id = location.rsplit_once('/').unwrap().1;

            Ok(SearchInfo::new(
                self,
                search_id.to_string(),
                location.to_string(),
            ))?
        } else {
            Err(Error::UrlNotFound)
        }
    }

    pub async fn read_search_info(
        &self,
        container_name: &str,
        search_id: &str,
    ) -> Result<Value, reqwest::Error> {
        let url = format!(
            "{base}/services/{container_name}/search/{search_id}/info",
            base = &self.base_url
        );
        Ok(self.client_get_json(&url).await?)
    }

    pub async fn read_search_result_page(
        &self,
        container_name: &str,
        search_id: &str,
        page: Option<u32>,
    ) -> Result<Value, reqwest::Error> {
        let search_url = format!(
            "{base}/services/{container_name}/search/{search_id}",
            base = &self.base_url
        );
        let params = [("page", page.unwrap_or(0).to_string())];
        let url = reqwest::Url::parse_with_params(&search_url, &params).unwrap();
        println!("read_search_result_page: url={:?}", url);

        Ok(self.client.get(url).send().await?.json().await?)
    }

    pub async fn read_search_result_annotations(
        &self,
        container_name: &str,
        search_id: &str,
        start_page: Option<u32>,
    ) -> Result<AnnoIter, Error> {
        Ok(AnnoIter::new(
            self,
            container_name,
            search_id,
            start_page.unwrap_or(0),
        ))?
        .await
    }

    pub async fn foreach_search_result_annotation(
        &self,
        container_name: &str,
        search_id: &str,
        start_page: Option<u32>,
        f: &dyn Fn(&Value) -> (),
    ) -> Result<(), Error> {
        let annotation_page = &self
            .read_search_result_page(container_name, search_id, start_page)
            .await
            .unwrap();
        if let Array(annos) = &annotation_page["items"] {
            for anno in annos {
                f(&anno);
            }
            Ok(())
        } else {
            Err(Error::MalformedAnnotationPage(annotation_page.clone()))
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
pub struct AnnoIter<'a> {
    client: &'a AnnoRepoClient,
    url: String,
    cur_page: u32,
    cur_anno: usize,
    annotations: VecDeque<Value>,
}

impl<'a> AnnoIter<'a> {
    pub async fn new(
        client: &'a AnnoRepoClient,
        container_name: &str,
        search_id: &str,
        start_page: u32,
    ) -> Result<Self, Error> {
        let search_url = format!(
            "{base}/services/{container_name}/search/{search_id}",
            base = client.base_url
        );
        let mut annotation_page = client
            .read_search_result_page(container_name, search_id, Some(start_page))
            .await
            .unwrap();
        let item = annotation_page["items"].take();
        // if let Array(annos) = annotation_page["items"].take() {
        if let Array(annos) = item {
            Ok(Self {
                client,
                url: search_url,
                cur_page: start_page,
                cur_anno: 0,
                annotations: annos.into(),
            })
        } else {
            Err(Error::MalformedAnnotationPage(annotation_page))
        }
    }
}

impl<'a> Iterator for AnnoIter<'a> {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        println!("cur={}, size={}", self.cur_anno, self.annotations.len());
        // if self.cur_anno < self.annotations.len() {
        //     let anno = self.annotations.get(self.cur_anno).unwrap().clone();
        //     self.cur_anno += 1;
        //     return Some(anno);
        // }
        while let Some(anno) = self.annotations.pop_front() {
            println!("cur={}, left={}", anno, self.annotations.len());
            return Some(anno);
        }
        None
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
