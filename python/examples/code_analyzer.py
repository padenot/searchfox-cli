#!/usr/bin/env python3
"""
Code Analyzer Example

A practical example showing how to use searchfox to analyze code patterns
and relationships in the Mozilla codebase.
"""

import searchfox
import re
from collections import defaultdict
from typing import List, Dict, Tuple


class MozillaCodeAnalyzer:
    """Analyzer for Mozilla codebase using searchfox API."""

    def __init__(self, repo="mozilla-central"):
        self.client = searchfox.SearchfoxClient(repo)
        self.repo = repo

    def find_implementations(self, interface: str, limit: int = 20) -> List[Dict]:
        """Find all implementations of a given interface."""
        # Use indexed symbol search for the interface
        results = self.client.search(
            symbol=interface,
            cpp=True,
            limit=limit
        )

        implementations = []
        for path, line_num, line in results:
            # Parse class name from the line
            match = re.search(r'class\s+(\w+)', line)
            if match:
                implementations.append({
                    'class': match.group(1),
                    'path': path,
                    'line': line_num,
                    'full_line': line.strip()
                })

        return implementations

    def analyze_include_patterns(self, directory: str, limit: int = 100) -> Dict[str, int]:
        """Analyze include patterns in a directory."""
        # Note: This is a text search, but limited to a specific directory
        # For better performance, consider using local ripgrep
        results = self.client.search(
            query='#include',
            path=f"^{directory}",
            cpp=True,
            limit=limit
        )

        include_counts = defaultdict(int)
        for _, _, line in results:
            # Extract header file from include statement
            match = re.search(r'#include\s+[<"]([^>"]+)[>"]', line)
            if match:
                header = match.group(1)
                include_counts[header] += 1

        return dict(sorted(include_counts.items(), key=lambda x: x[1], reverse=True))

    def find_method_calls(self, class_name: str, method_name: str, limit: int = 50) -> List[Tuple[str, int, str]]:
        """Find all calls to a specific method of a class."""
        # Use indexed symbol search for the method
        full_method = f"{class_name}::{method_name}"
        results = self.client.search(
            symbol=full_method,
            cpp=True,
            limit=limit
        )

        all_results = results

        # If no results, try searching just for the method name
        if not results:
            results = self.client.search(
                id=method_name,
                cpp=True,
                limit=limit
            )
            all_results = results

        # Remove duplicates
        seen = set()
        unique_results = []
        for result in all_results:
            key = (result[0], result[1])  # (path, line_num)
            if key not in seen:
                seen.add(key)
                unique_results.append(result)

        return unique_results

    def get_class_hierarchy(self, base_class: str, depth: int = 2) -> Dict:
        """Build a class hierarchy tree starting from a base class."""
        hierarchy = {
            'class': base_class,
            'derived': []
        }

        # Find direct derived classes
        implementations = self.find_implementations(base_class, limit=30)

        if depth > 0:
            for impl in implementations:
                # Recursively find derived classes
                sub_hierarchy = self.get_class_hierarchy(impl['class'], depth - 1)
                hierarchy['derived'].append(sub_hierarchy)
        else:
            hierarchy['derived'] = [{'class': impl['class'], 'derived': []} for impl in implementations]

        return hierarchy

    def analyze_api_usage(self, api_function: str) -> Dict:
        """Analyze how an API function is used across the codebase."""
        # Use indexed symbol search for better performance
        results = self.client.search(
            symbol=api_function,
            limit=100
        )

        usage_stats = {
            'total_calls': len(results),
            'files': set(),
            'directories': defaultdict(int),
            'patterns': defaultdict(int)
        }

        for path, _, line in results:
            usage_stats['files'].add(path)

            # Count by directory
            if '/' in path:
                directory = path.split('/')[0]
                usage_stats['directories'][directory] += 1

            # Detect usage patterns
            if f'new {api_function}' in line:
                usage_stats['patterns']['constructor'] += 1
            elif f'{api_function}::' in line:
                usage_stats['patterns']['static_method'] += 1
            elif f'->{api_function}' in line or f'.{api_function}' in line:
                usage_stats['patterns']['method_call'] += 1
            else:
                usage_stats['patterns']['other'] += 1

        usage_stats['files'] = len(usage_stats['files'])
        usage_stats['directories'] = dict(usage_stats['directories'])
        usage_stats['patterns'] = dict(usage_stats['patterns'])

        return usage_stats


def print_hierarchy(hierarchy: Dict, indent: int = 0):
    """Pretty print a class hierarchy."""
    print("  " * indent + f"├─ {hierarchy['class']}")
    for derived in hierarchy['derived']:
        print_hierarchy(derived, indent + 1)


def main():
    """Run example analyses."""
    print("Mozilla Code Analyzer")
    print("=" * 60)

    analyzer = MozillaCodeAnalyzer()

    # Example 1: Find AudioNode implementations
    print("\n1. Finding AudioNode implementations:")
    print("-" * 40)
    implementations = analyzer.find_implementations("AudioNode", limit=10)
    for impl in implementations[:5]:
        print(f"  {impl['class']:<20} in {impl['path']}:{impl['line']}")

    # Example 2: Analyze include patterns in dom/media
    print("\n2. Most included headers in dom/media:")
    print("-" * 40)
    includes = analyzer.analyze_include_patterns("dom/media", limit=50)
    for header, count in list(includes.items())[:10]:
        print(f"  {header:<40} {count} times")

    # Example 3: Find method calls
    print("\n3. Finding calls to AudioContext::CreateGain:")
    print("-" * 40)
    calls = analyzer.find_method_calls("AudioContext", "CreateGain", limit=10)
    for path, line_num, line in calls[:5]:
        print(f"  {path}:{line_num}")
        print(f"    {line.strip()}")

    # Example 4: Build class hierarchy
    print("\n4. EventTarget class hierarchy (depth=1):")
    print("-" * 40)
    hierarchy = analyzer.get_class_hierarchy("EventTarget", depth=1)
    print_hierarchy(hierarchy)

    # Example 5: Analyze API usage
    print("\n5. Analyzing usage of 'createElement':")
    print("-" * 40)
    usage = analyzer.analyze_api_usage("createElement")
    print(f"  Total calls: {usage['total_calls']}")
    print(f"  Files using it: {usage['files']}")
    print(f"  Top directories:")
    for directory, count in sorted(usage['directories'].items(), key=lambda x: x[1], reverse=True)[:5]:
        print(f"    {directory:<20} {count} calls")
    print(f"  Usage patterns:")
    for pattern, count in usage['patterns'].items():
        print(f"    {pattern:<20} {count} times")

    # Example 6: Find security-sensitive patterns
    print("\n6. Security Analysis - Finding unsafe patterns:")
    print("-" * 40)
    unsafe_patterns = [
        ("strcpy", "Use of unsafe strcpy"),
        ("sprintf", "Use of unsafe sprintf"),
        ("gets", "Use of unsafe gets"),
    ]

    for pattern, description in unsafe_patterns:
        results = analyzer.client.search(query=pattern, c_lang=True, limit=5)
        if results:
            print(f"  ⚠️  {description}: {len(results)} occurrences")
            for path, line_num, _ in results[:2]:
                print(f"     {path}:{line_num}")

    print("\n" + "=" * 60)
    print("Analysis complete!")


if __name__ == "__main__":
    main()