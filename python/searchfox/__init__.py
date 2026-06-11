"""Python bindings for searchfox.org API."""

try:
    from searchfox.searchfox import (
        AsyncSearchfoxClient,
        SearchfoxClient,
        SearchfoxError,
        SearchfoxNetworkError,
        SearchfoxRequestError,
    )
except ImportError:
    from .searchfox import (
        AsyncSearchfoxClient,
        SearchfoxClient,
        SearchfoxError,
        SearchfoxNetworkError,
        SearchfoxRequestError,
    )


class Lang:
    """Language filter constants for use with search().

    Pass as the ``lang`` argument, e.g. ``client.search(lang=Lang.Cpp)``.
    String values are accepted directly too: ``client.search(lang="cpp")``.
    "c" and "kotlin" are accepted as aliases for Cpp and Java respectively.
    """

    Cpp = "cpp"
    Js = "js"
    WebIdl = "webidl"
    Java = "java"
    Rust = "rust"
    Python = "python"
    Html = "html"
    Css = "css"


__version__ = "0.7.0"
__all__ = [
    "SearchfoxClient",
    "AsyncSearchfoxClient",
    "Lang",
    "SearchfoxError",
    "SearchfoxNetworkError",
    "SearchfoxRequestError",
    "search",
    "get_file",
    "get_definition",
    "get_blame_for_lines",
]


def search(
    query=None,
    repo="mozilla-central",
    path=None,
    case=False,
    regexp=False,
    limit=50,
    context=None,
    symbol=None,
    id=None,
    langs=None,
    tests=None,
    log_requests=False,
):
    """Search Mozilla codebases using searchfox.org.

    Args:
        query: Search query string.
        repo: Repository to search in (default: mozilla-central).
        path: Filter results by path prefix.
        case: Enable case-sensitive search.
        regexp: Enable regular expression search.
        limit: Maximum number of results (default: 50).
        context: Number of context lines around matches.
        symbol: Search for symbol definitions.
        id: Search for exact identifier matches.
        langs: Language filter as a list, e.g. [Lang.Cpp] or ["cpp", "webidl"].
            Accepts "cpp", "c", "js", "webidl", "java", "kotlin", "rust",
            "python", "html", "css". Multiple values are OR-ed.
        tests: "only" to restrict to test files, "exclude" to omit them.
        log_requests: Enable request logging.

    Returns:
        List of tuples (path, line_number, line_content).
    """
    client = SearchfoxClient(repo, log_requests)
    return client.search(
        query=query,
        path=path,
        case=case,
        regexp=regexp,
        limit=limit,
        context=context,
        symbol=symbol,
        id=id,
        langs=langs,
        tests=tests,
    )


def get_file(path, repo="mozilla-central", log_requests=False):
    """Get the contents of a file from a Mozilla repository.

    Args:
        path: Path to the file relative to repository root.
        repo: Repository name (default: mozilla-central).
        log_requests: Enable request logging.

    Returns:
        File contents as string.
    """
    client = SearchfoxClient(repo, log_requests)
    return client.get_file(path)


def get_definition(symbol, repo="mozilla-central", path_filter=None, log_requests=False):
    """Get the definition of a symbol.

    Args:
        symbol: Symbol name to find definition for.
        repo: Repository name (default: mozilla-central).
        path_filter: Optional path filter.
        log_requests: Enable request logging.

    Returns:
        Definition source as string.
    """
    client = SearchfoxClient(repo, log_requests)
    return client.get_definition(symbol, path_filter)


def get_blame_for_lines(path, lines, repo="mozilla-central", log_requests=False):
    """Get blame information for specific lines in a file.

    Args:
        path: Path to the file relative to repository root.
        lines: List of line numbers to get blame for.
        repo: Repository name (default: mozilla-central).
        log_requests: Enable request logging.

    Returns:
        List of tuples (line_number, commit_hash, message, date).
    """
    client = SearchfoxClient(repo, log_requests)
    return client.get_blame_for_lines(path, lines)
