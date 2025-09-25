#!/usr/bin/env python3
"""
Searchfox Python API Demo

This example demonstrates various features of the searchfox Python bindings.
Before running, make sure to build and install the package:
    pip install maturin
    maturin develop
"""

import searchfox
import json
import sys


def separator(title):
    """Print a section separator."""
    print(f"\n{'=' * 60}")
    print(f"  {title}")
    print('=' * 60)


def main():
    # Create a client for mozilla-central repository
    client = searchfox.SearchfoxClient("mozilla-central")

    # Example 1: Indexed symbol search (efficient)
    separator("Indexed Symbol Search: Find AudioContext symbols")
    results = client.search(symbol="AudioContext", limit=5)
    for path, line_num, line in results:
        print(f"{path}:{line_num}: {line}")
    print(f"Found {len(results)} results")

    # Example 2: Language-filtered indexed search
    separator("C++ Only Search: AudioNode symbols in dom/media")
    results = client.search(
        symbol="AudioNode",
        path="^dom/media/webaudio",
        cpp=True,
        limit=3
    )
    for path, line_num, line in results:
        print(f"{path}:{line_num}: {line}")

    # Example 3: Symbol search
    separator("Symbol Search: AudioContext methods")
    results = client.search(
        symbol="AudioContext",
        limit=5
    )
    for path, line_num, line in results:
        print(f"{path}:{line_num}: {line}")

    # Example 4: Get a specific file
    separator("File Retrieval: Get README from dom/media")
    try:
        content = client.get_file("dom/media/README")
        lines = content.split('\n')
        print(f"File has {len(lines)} lines")
        print("First 5 lines:")
        for i, line in enumerate(lines[:5], 1):
            print(f"  {i}: {line}")
    except Exception as e:
        print(f"Could not retrieve file: {e}")

    # Example 5: Find definition
    separator("Definition Search: AudioContext::CreateGain")
    definition = client.get_definition("AudioContext::CreateGain")
    if definition:
        print(definition)
    else:
        print("Definition not found")

    # Example 6: Using convenience functions
    separator("Convenience Functions")

    # Quick indexed search
    print("\nQuick indexed search for 'malloc' symbol in C files:")
    results = searchfox.search(symbol="malloc", c_lang=True, limit=3)
    for path, line_num, line in results[:3]:
        print(f"  {path}:{line_num}")

    # Quick file retrieval
    print("\nGet a WebIDL file:")
    try:
        content = searchfox.get_file("dom/webidl/AudioContext.webidl")
        lines = content.split('\n')
        # Find interface definition
        for i, line in enumerate(lines):
            if 'interface AudioContext' in line:
                print(f"Found interface definition at line {i+1}:")
                print(f"  {line}")
                break
    except Exception as e:
        print(f"Could not retrieve WebIDL file: {e}")

    # Example 7: Call graph analysis
    separator("Call Graph: Functions called by AudioContext::CreateGain")
    try:
        call_graph = client.search_call_graph(
            calls_from="mozilla::dom::AudioContext::CreateGain",
            depth=2
        )
        if call_graph:
            # Parse and display results
            data = json.loads(call_graph)
            if isinstance(data, dict) and len(data) > 0:
                print("Call graph data retrieved successfully")
                print(f"Keys in response: {list(data.keys())[:5]}...")
            else:
                print("No call graph data found")
    except Exception as e:
        print(f"Call graph search failed: {e}")

    # Example 8: Different repository
    separator("Search in Different Repository: Thunderbird")
    tb_client = searchfox.SearchfoxClient("comm-central")
    results = tb_client.search(query="MailServices", limit=3)
    for path, line_num, line in results:
        print(f"{path}:{line_num}: {line}")

    # Example 9: Indexed identifier search
    separator("Indexed ID Search: Exact identifier 'CreateGain'")
    results = client.search(
        id="CreateGain",
        path="dom/media",
        limit=5
    )
    for path, line_num, line in results:
        if "Create" in line:  # Filter to actual matches
            print(f"{path}:{line_num}: {line.strip()}")

    # Example 10: Performance monitoring
    separator("Performance: Measure search latency")
    client_with_logging = searchfox.SearchfoxClient("mozilla-central", log_requests=True)
    print("Pinging searchfox.org...")
    latency = client_with_logging.ping()
    print(f"Baseline latency: {latency:.3f} seconds")

    print("\nSearching with request logging enabled (using indexed search)...")
    results = client_with_logging.search(symbol="test", limit=1)
    print(f"Search completed, found {len(results)} results")


def advanced_example():
    """Advanced example: Building a simple code explorer."""

    separator("Advanced Example: Interactive Code Explorer")

    client = searchfox.SearchfoxClient("mozilla-central")

    # Find AudioNode symbol definitions
    print("\nFinding AudioNode symbol definitions...")
    results = client.search(
        symbol="AudioNode",
        cpp=True,
        limit=10
    )

    audio_nodes = []
    for path, line_num, line in results:
        # Extract class name from the line
        if "class " in line and ": public AudioNode" in line:
            parts = line.split()
            if "class" in parts:
                idx = parts.index("class")
                if idx + 1 < len(parts):
                    class_name = parts[idx + 1].rstrip(":")
                    audio_nodes.append({
                        'name': class_name,
                        'path': path,
                        'line': line_num
                    })

    print(f"\nFound {len(audio_nodes)} AudioNode subclasses:")
    for node in audio_nodes[:5]:  # Show first 5
        print(f"  - {node['name']} at {node['path']}:{node['line']}")

    # Get detailed info about one of them
    if audio_nodes:
        selected = audio_nodes[0]
        print(f"\nGetting definition of {selected['name']}...")
        definition = client.get_definition(selected['name'])
        if definition:
            lines = definition.split('\n')
            print(f"Definition preview (first 10 lines):")
            for line in lines[:10]:
                print(line)


if __name__ == "__main__":
    try:
        print("Searchfox Python API Demo")
        print("=" * 60)

        # Run main demo
        main()

        # Optionally run advanced example
        if len(sys.argv) > 1 and sys.argv[1] == "--advanced":
            advanced_example()

        print("\n" + "=" * 60)
        print("Demo completed successfully!")

    except ImportError:
        print("Error: searchfox module not found.")
        print("Please build and install the package first:")
        print("  pip install maturin")
        print("  maturin develop")
        sys.exit(1)
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)