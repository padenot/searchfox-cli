"""Python bindings for searchfox.org API."""

# The compiled module is installed as searchfox.searchfox
try:
    # When installed via maturin
    from searchfox.searchfox import SearchfoxClient
except ImportError:
    # During development with just the .so file
    from .searchfox import SearchfoxClient

__version__ = "0.2.0"
__all__ = ["SearchfoxClient", "search", "get_file", "get_definition"]


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
    cpp=False,
    c_lang=False,
    webidl=False,
    js=False,
    log_requests=False,
):
    """
    Search Mozilla codebases using searchfox.org.

    Args:
        query: Search query string
        repo: Repository to search in (default: mozilla-central)
        path: Filter results by path prefix
        case: Enable case-sensitive search
        regexp: Enable regular expression search
        limit: Maximum number of results (default: 50)
        context: Number of context lines around matches
        symbol: Search for symbol definitions
        id: Search for exact identifier matches
        cpp: Filter to C++ files only
        c_lang: Filter to C files only
        webidl: Filter to WebIDL files only
        js: Filter to JavaScript files only
        log_requests: Enable request logging

    Returns:
        List of tuples (path, line_number, line_content)
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
        cpp=cpp,
        c_lang=c_lang,
        webidl=webidl,
        js=js,
    )


def get_file(path, repo="mozilla-central", log_requests=False):
    """
    Get the contents of a file from a Mozilla repository.

    Args:
        path: Path to the file relative to repository root
        repo: Repository name (default: mozilla-central)
        log_requests: Enable request logging

    Returns:
        File contents as string
    """
    client = SearchfoxClient(repo, log_requests)
    return client.get_file(path)


def get_definition(symbol, repo="mozilla-central", path_filter=None, log_requests=False):
    """
    Get the definition of a symbol.

    Args:
        symbol: Symbol name to find definition for
        repo: Repository name (default: mozilla-central)
        path_filter: Optional path filter
        log_requests: Enable request logging

    Returns:
        Definition context as string
    """
    client = SearchfoxClient(repo, log_requests)
    return client.get_definition(symbol, path_filter)