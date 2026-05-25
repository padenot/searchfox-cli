#![allow(non_local_definitions)]

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;
use searchfox_lib::{
    call_graph::CallGraphQuery, can_gc::GcInfo, field_layout::FieldLayoutQuery,
    search::SearchOptions, CategoryFilter, Lang, SearchfoxClient as RustClient,
};
use std::sync::Arc;
use tokio::runtime::Runtime;

fn parse_langs(langs: Option<Vec<String>>) -> PyResult<Vec<Lang>> {
    let Some(langs) = langs else { return Ok(Vec::new()) };
    langs
        .iter()
        .map(|s| {
            Lang::from_str(s).ok_or_else(|| {
                PyException::new_err(format!(
                    "Unknown language '{}': expected one of cpp, c, js, webidl, java, kotlin, rust, python, html, css",
                    s
                ))
            })
        })
        .collect()
}

fn parse_category_filter(tests: Option<&str>) -> PyResult<CategoryFilter> {
    match tests {
        None | Some("all") => Ok(CategoryFilter::All),
        Some("only") => Ok(CategoryFilter::OnlyTests),
        Some("exclude") => Ok(CategoryFilter::ExcludeTests),
        Some(v) => Err(PyException::new_err(format!(
            "Invalid tests value '{}': expected 'only', 'exclude', or None",
            v
        ))),
    }
}

// ---------------------------------------------------------------------------
// Synchronous client
// ---------------------------------------------------------------------------

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

    #[pyo3(signature = (query=None, path=None, case=None, regexp=None, limit=None, context=None, symbol=None, id=None, langs=None, tests=None))]
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
        langs: Option<Vec<String>>,
        tests: Option<String>,
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
            lang: parse_langs(langs)?,
            category_filter: parse_category_filter(tests.as_deref())?,
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

    #[pyo3(signature = (symbol, path_filter=None))]
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

    #[pyo3(signature = (calls_from=None, calls_to=None, calls_between=None, depth=None))]
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

    fn search_field_layout(&self, py: Python<'_>, class_name: String) -> PyResult<String> {
        let query = FieldLayoutQuery { class_name };

        let client = self.inner.clone();
        let result = py.allow_threads(|| {
            self.runtime
                .block_on(async move { client.search_field_layout(&query).await })
        });

        match result {
            Ok(json) => {
                Ok(serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string()))
            }
            Err(e) => Err(PyException::new_err(format!(
                "Field layout search failed: {}",
                e
            ))),
        }
    }

    fn get_gc_info(
        &self,
        py: Python<'_>,
        symbol: String,
    ) -> PyResult<Vec<(String, String, bool, Option<String>)>> {
        let client = self.inner.clone();
        let result = py.allow_threads(|| {
            self.runtime
                .block_on(async move { client.get_gc_info(&symbol).await })
        });

        match result {
            Ok(infos) => Ok(infos
                .into_iter()
                .map(
                    |GcInfo {
                         pretty,
                         mangled,
                         can_gc,
                         gc_path,
                     }| (pretty, mangled, can_gc, gc_path),
                )
                .collect()),
            Err(e) => Err(PyException::new_err(format!(
                "GC info lookup failed: {}",
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

    fn get_blame_for_lines(
        &self,
        py: Python<'_>,
        path: String,
        lines: Vec<usize>,
    ) -> PyResult<Vec<(usize, String, String, String)>> {
        let client = self.inner.clone();
        let result = py.allow_threads(|| {
            self.runtime
                .block_on(async move { client.get_blame_for_lines(&path, &lines).await })
        });

        match result {
            Ok(blame_map) => {
                let mut results = Vec::new();
                for (line_num, blame_info) in blame_map {
                    if let Some(commit_info) = blame_info.commit_info {
                        let parsed = searchfox_lib::parse_commit_header(&commit_info.header);
                        let message = if let Some(bug) = parsed.bug_number {
                            format!("Bug {}: {}", bug, parsed.message)
                        } else {
                            parsed.message.clone()
                        };
                        results.push((
                            line_num,
                            blame_info.commit_hash[..8].to_string(),
                            message,
                            parsed.date,
                        ));
                    }
                }
                results.sort_by_key(|(line_num, _, _, _)| *line_num);
                Ok(results)
            }
            Err(e) => Err(PyException::new_err(format!("Failed to get blame: {}", e))),
        }
    }
}

// ---------------------------------------------------------------------------
// Asynchronous client
// ---------------------------------------------------------------------------

#[pyclass]
struct AsyncSearchfoxClient {
    inner: Arc<RustClient>,
}

#[pymethods]
impl AsyncSearchfoxClient {
    #[new]
    #[pyo3(signature = (repo="mozilla-central", log_requests=false))]
    fn new(repo: &str, log_requests: bool) -> PyResult<Self> {
        let client = RustClient::new(repo.to_string(), log_requests)
            .map_err(|e| PyException::new_err(format!("Failed to create client: {}", e)))?;

        Ok(Self {
            inner: Arc::new(client),
        })
    }

    #[pyo3(signature = (query=None, path=None, case=None, regexp=None, limit=None, context=None, symbol=None, id=None, langs=None, tests=None))]
    fn search<'py>(
        &self,
        py: Python<'py>,
        query: Option<String>,
        path: Option<String>,
        case: Option<bool>,
        regexp: Option<bool>,
        limit: Option<usize>,
        context: Option<usize>,
        symbol: Option<String>,
        id: Option<String>,
        langs: Option<Vec<String>>,
        tests: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let options = SearchOptions {
            query,
            path,
            case: case.unwrap_or(false),
            regexp: regexp.unwrap_or(false),
            limit: limit.unwrap_or(50),
            context,
            symbol,
            id,
            lang: parse_langs(langs)?,
            category_filter: parse_category_filter(tests.as_deref())?,
        };

        let client = self.inner.clone();
        future_into_py(py, async move {
            let results = client
                .search(&options)
                .await
                .map_err(|e| PyException::new_err(format!("Search failed: {}", e)))?;

            Ok(results
                .into_iter()
                .map(|r| (r.path, r.line_number, r.line))
                .collect::<Vec<_>>())
        })
    }

    fn get_file<'py>(&self, py: Python<'py>, path: String) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        future_into_py(py, async move {
            client
                .get_file(&path)
                .await
                .map_err(|e| PyException::new_err(format!("Failed to get file: {}", e)))
        })
    }

    #[pyo3(signature = (symbol, path_filter=None))]
    fn get_definition<'py>(
        &self,
        py: Python<'py>,
        symbol: String,
        path_filter: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        let options = SearchOptions::default();
        future_into_py(py, async move {
            client
                .find_and_display_definition(&symbol, path_filter.as_deref(), &options)
                .await
                .map_err(|e| PyException::new_err(format!("Failed to get definition: {}", e)))
        })
    }

    #[pyo3(signature = (calls_from=None, calls_to=None, calls_between=None, depth=None))]
    fn search_call_graph<'py>(
        &self,
        py: Python<'py>,
        calls_from: Option<String>,
        calls_to: Option<String>,
        calls_between: Option<(String, String)>,
        depth: Option<u32>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let query = CallGraphQuery {
            calls_from,
            calls_to,
            calls_between,
            depth: depth.unwrap_or(2),
        };

        let client = self.inner.clone();
        future_into_py(py, async move {
            let json = client
                .search_call_graph(&query)
                .await
                .map_err(|e| PyException::new_err(format!("Call graph search failed: {}", e)))?;
            Ok(serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string()))
        })
    }

    fn search_field_layout<'py>(
        &self,
        py: Python<'py>,
        class_name: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let query = FieldLayoutQuery { class_name };
        let client = self.inner.clone();
        future_into_py(py, async move {
            let json = client
                .search_field_layout(&query)
                .await
                .map_err(|e| PyException::new_err(format!("Field layout search failed: {}", e)))?;
            Ok(serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string()))
        })
    }

    fn get_gc_info<'py>(&self, py: Python<'py>, symbol: String) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        future_into_py(py, async move {
            let infos = client
                .get_gc_info(&symbol)
                .await
                .map_err(|e| PyException::new_err(format!("GC info lookup failed: {}", e)))?;
            Ok(infos
                .into_iter()
                .map(
                    |GcInfo {
                         pretty,
                         mangled,
                         can_gc,
                         gc_path,
                     }| (pretty, mangled, can_gc, gc_path),
                )
                .collect::<Vec<_>>())
        })
    }

    fn ping<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        future_into_py(py, async move {
            let duration = client
                .ping()
                .await
                .map_err(|e| PyException::new_err(format!("Ping failed: {}", e)))?;
            Ok(duration.as_secs_f64())
        })
    }

    fn get_blame_for_lines<'py>(
        &self,
        py: Python<'py>,
        path: String,
        lines: Vec<usize>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        future_into_py(py, async move {
            let blame_map = client
                .get_blame_for_lines(&path, &lines)
                .await
                .map_err(|e| PyException::new_err(format!("Failed to get blame: {}", e)))?;

            let mut results = Vec::new();
            for (line_num, blame_info) in blame_map {
                if let Some(commit_info) = blame_info.commit_info {
                    let parsed = searchfox_lib::parse_commit_header(&commit_info.header);
                    let message = if let Some(bug) = parsed.bug_number {
                        format!("Bug {}: {}", bug, parsed.message)
                    } else {
                        parsed.message.clone()
                    };
                    results.push((
                        line_num,
                        blame_info.commit_hash[..8].to_string(),
                        message,
                        parsed.date,
                    ));
                }
            }
            results.sort_by_key(|(line_num, _, _, _)| *line_num);
            Ok(results)
        })
    }
}

// ---------------------------------------------------------------------------

#[pymodule]
fn searchfox(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SearchfoxClient>()?;
    m.add_class::<AsyncSearchfoxClient>()?;
    Ok(())
}
