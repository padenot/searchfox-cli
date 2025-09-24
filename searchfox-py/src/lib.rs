use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use searchfox_lib::{
    call_graph::CallGraphQuery, search::SearchOptions, SearchfoxClient as RustClient,
};
use std::sync::Arc;
use tokio::runtime::Runtime;

#[pyclass]
struct SearchfoxClient {
    inner: Arc<RustClient>,
    runtime: Arc<Runtime>,
}

#[pymethods]
impl SearchfoxClient {
    #[new]
    #[pyo3(signature = (repo="mozilla-central", log_requests=false))]
    fn new(repo: &str, log_requests: bool) -> PyResult<Self> {
        let runtime = Runtime::new()
            .map_err(|e| PyException::new_err(format!("Failed to create runtime: {}", e)))?;

        let client = RustClient::new(repo.to_string(), log_requests)
            .map_err(|e| PyException::new_err(format!("Failed to create client: {}", e)))?;

        Ok(Self {
            inner: Arc::new(client),
            runtime: Arc::new(runtime),
        })
    }

    fn search(
        &self,
        py: Python<'_>,
        query: Option<String>,
        path: Option<String>,
        case: Option<bool>,
        regexp: Option<bool>,
        limit: Option<usize>,
        context: Option<usize>,
        symbol: Option<String>,
        id: Option<String>,
        cpp: Option<bool>,
        c_lang: Option<bool>,
        webidl: Option<bool>,
        js: Option<bool>,
    ) -> PyResult<Vec<(String, usize, String)>> {
        let options = SearchOptions {
            query,
            path,
            case: case.unwrap_or(false),
            regexp: regexp.unwrap_or(false),
            limit: limit.unwrap_or(50),
            context,
            symbol,
            id,
            cpp: cpp.unwrap_or(false),
            c_lang: c_lang.unwrap_or(false),
            webidl: webidl.unwrap_or(false),
            js: js.unwrap_or(false),
        };

        let client = self.inner.clone();
        let results = py.allow_threads(|| {
            self.runtime
                .block_on(async move { client.search(&options).await })
        });

        match results {
            Ok(results) => {
                let py_results = results
                    .into_iter()
                    .map(|r| (r.path, r.line_number, r.line))
                    .collect();
                Ok(py_results)
            }
            Err(e) => Err(PyException::new_err(format!("Search failed: {}", e))),
        }
    }

    fn get_file(&self, py: Python<'_>, path: String) -> PyResult<String> {
        let client = self.inner.clone();
        let result = py.allow_threads(|| {
            self.runtime
                .block_on(async move { client.get_file(&path).await })
        });

        match result {
            Ok(content) => Ok(content),
            Err(e) => Err(PyException::new_err(format!("Failed to get file: {}", e))),
        }
    }

    fn get_definition(
        &self,
        py: Python<'_>,
        symbol: String,
        path_filter: Option<String>,
    ) -> PyResult<String> {
        let client = self.inner.clone();
        let options = SearchOptions::default();

        let result = py.allow_threads(|| {
            self.runtime.block_on(async move {
                client
                    .find_and_display_definition(&symbol, path_filter.as_deref(), &options)
                    .await
            })
        });

        match result {
            Ok(definition) => Ok(definition),
            Err(e) => Err(PyException::new_err(format!(
                "Failed to get definition: {}",
                e
            ))),
        }
    }

    fn search_call_graph(
        &self,
        py: Python<'_>,
        calls_from: Option<String>,
        calls_to: Option<String>,
        calls_between: Option<(String, String)>,
        depth: Option<u32>,
    ) -> PyResult<String> {
        let query = CallGraphQuery {
            calls_from,
            calls_to,
            calls_between,
            depth: depth.unwrap_or(2),
        };

        let client = self.inner.clone();
        let result = py.allow_threads(|| {
            self.runtime
                .block_on(async move { client.search_call_graph(&query).await })
        });

        match result {
            Ok(json) => {
                Ok(serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string()))
            }
            Err(e) => Err(PyException::new_err(format!(
                "Call graph search failed: {}",
                e
            ))),
        }
    }

    fn ping(&self, py: Python<'_>) -> PyResult<f64> {
        let client = self.inner.clone();
        let result = py.allow_threads(|| self.runtime.block_on(async move { client.ping().await }));

        match result {
            Ok(duration) => Ok(duration.as_secs_f64()),
            Err(e) => Err(PyException::new_err(format!("Ping failed: {}", e))),
        }
    }
}

#[pymodule]
fn searchfox(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<SearchfoxClient>()?;
    Ok(())
}
