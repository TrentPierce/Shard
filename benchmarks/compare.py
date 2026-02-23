#!/usr/bin/env python3
"""
Shard Benchmark Comparison Script

Compares performance between OpenAI API and local Shard network.
Measures: time-to-first-token, total latency, tokens/second.
"""

import argparse
import json
import os
import sys
import time
from dataclasses import dataclass, asdict
from datetime import datetime
from typing import Optional

import requests


@dataclass
class BenchmarkResult:
    """Result of a single benchmark run."""
    provider: str  # "openai" or "shard"
    prompt: str
    time_to_first_token_ms: float
    total_latency_ms: float
    tokens_generated: int
    tokens_per_second: float
    success: bool
    error: Optional[str] = None


@dataclass
class ComparisonSummary:
    """Summary comparison between providers."""
    openai_avg_ttft_ms: float
    shard_avg_ttft_ms: float
    openai_avg_latency_ms: float
    shard_avg_latency_ms: float
    openai_avg_tps: float
    shard_avg_tps: float
    speedup_factor: float
    cost_savings_percent: float


# Enterprise-relevant test prompts (no "hello world")
TEST_PROMPTS = [
    {
        "category": "summarization",
        "prompt": """Summarize the following article in 3 bullet points:

The field of distributed computing has undergone significant transformations over the past decade. 
Cloud-native architectures have become the standard for deploying large-scale applications, 
enabling organizations to scale their operations dynamically. Container orchestration platforms 
like Kubernetes have simplified the deployment and management of complex microservices architectures.
Edge computing has emerged as a critical paradigm for processing data closer to its source,
reducing latency and bandwidth requirements for real-time applications."""
    },
    {
        "category": "code_generation",
        "prompt": """Write a Python function that implements binary search on a sorted array.
Include type hints and a docstring. The function should return the index if found, 
or -1 if not found."""
    },
    {
        "category": "code_review",
        "prompt": """Review the following Python code for potential bugs and performance issues:

def process_data(items):
    results = []
    for item in items:
        if item['active']:
            filtered = [x for x in results if x['id'] != item['id']]
            results.append({**item, 'processed': True})
    return results"""
    },
    {
        "category": "qa",
        "prompt": """What are the key differences between OAuth 2.0 and OpenID Connect?
When would you choose one over the other for authentication?"""
    },
    {
        "category": "writing",
        "prompt": """Write a professional email to a client explaining that their project
timeline to be extended by needs 2 weeks due to unexpected technical complexities.
Keep it concise and constructive."""
    },
    {
        "category": "extraction",
        "prompt": """Extract all email addresses and phone numbers from the following text:
Contact John at john.doe@example.com or 555-123-4567. 
For Susan, reach out at susan.smith@company.org or 555.987.6543.
The main office is at 1-800-555-0199."""
    },
    {
        "category": "classification",
        "prompt": """Classify this feedback as Positive, Negative, or Neutral:
"The new dashboard is visually appealing but the load times have increased significantly.
I hope this gets addressed in the next update." """
    },
    {
        "category": "sql_generation",
        "prompt": """Write a SQL query to find the top 5 customers by total order value
in the last 30 days. Include customer name, email, and total spent."""
    },
    {
        "category": "explanation",
        "prompt": """Explain how a CDN (Content Delivery Network) improves website performance.
Mention at least 3 specific benefits."""
    },
    {
        "category": "reasoning",
        "prompt": """If all roses are flowers and some flowers fade quickly,
what can we conclude about roses? Explain your reasoning."""
    },
]


def call_openai(prompt: str, api_key: str, model: str = "gpt-4o-mini") -> BenchmarkResult:
    """Call OpenAI API and measure performance."""
    start_time = time.time()
    ttft = None
    
    try:
        response = requests.post(
            "https://api.openai.com/v1/chat/completions",
            headers={
                "Authorization": f"Bearer {api_key}",
                "Content-Type": "application/json",
            },
            json={
                "model": model,
                "messages": [{"role": "user", "content": prompt}],
                "stream": True,
            },
            stream=True,
            timeout=60,
        )
        
        full_response = ""
        for line in response.iter_lines():
            if not line:
                continue
            line = line.decode("utf-8")
            if line.startswith("data: "):
                data = line[6:]
                if data == "[DONE]":
                    break
                try:
                    chunk = json.loads(data)
                    content = chunk.get("choices", [{}])[0].get("delta", {}).get("content", "")
                    if content:
                        if ttft is None:
                            ttft = (time.time() - start_time) * 1000
                        full_response += content
                except json.JSONDecodeError:
                    continue
        
        total_latency = (time.time() - start_time) * 1000
        tokens = len(full_response.split())
        tps = (tokens / total_latency * 1000) if total_latency > 0 else 0
        
        return BenchmarkResult(
            provider="openai",
            prompt=prompt[:100] + "...",
            time_to_first_token_ms=ttft or 0,
            total_latency_ms=total_latency,
            tokens_generated=tokens,
            tokens_per_second=tps,
            success=True,
        )
    except Exception as e:
        return BenchmarkResult(
            provider="openai",
            prompt=prompt[:100] + "...",
            time_to_first_token_ms=0,
            total_latency_ms=0,
            tokens_generated=0,
            tokens_per_second=0,
            success=False,
            error=str(e),
        )


def call_shard(prompt: str, base_url: str = "http://localhost:9091") -> BenchmarkResult:
    """Call local Shard daemon and measure performance."""
    start_time = time.time()
    ttft = None
    
    try:
        response = requests.post(
            f"{base_url}/v1/chat/completions",
            headers={"Content-Type": "application/json"},
            json={
                "model": "shard-hybrid",
                "messages": [{"role": "user", "content": prompt}],
                "stream": True,
            },
            stream=True,
            timeout=60,
        )
        
        full_response = ""
        for line in response.iter_lines():
            if not line:
                continue
            line = line.decode("utf-8")
            if line.startswith("data: "):
                data = line[6:]
                if data == "[DONE]":
                    break
                try:
                    chunk = json.loads(data)
                    content = chunk.get("choices", [{}])[0].get("delta", {}).get("content", "")
                    if content:
                        if ttft is None:
                            ttft = (time.time() - start_time) * 1000
                        full_response += content
                except json.JSONDecodeError:
                    continue
        
        total_latency = (time.time() - start_time) * 1000
        tokens = len(full_response.split())
        tps = (tokens / total_latency * 1000) if total_latency > 0 else 0
        
        return BenchmarkResult(
            provider="shard",
            prompt=prompt[:100] + "...",
            time_to_first_token_ms=ttft or 0,
            total_latency_ms=total_latency,
            tokens_generated=tokens,
            tokens_per_second=tps,
            success=True,
        )
    except Exception as e:
        return BenchmarkResult(
            provider="shard",
            prompt=prompt[:100] + "...",
            time_to_first_token_ms=0,
            total_latency_ms=0,
            tokens_generated=0,
            tokens_per_second=0,
            success=False,
            error=str(e),
        )


def calculate_summary(openai_results: list, shard_results: list) -> ComparisonSummary:
    """Calculate summary statistics."""
    openai_success = [r for r in openai_results if r.success]
    shard_success = [r for r in shard_results if r.success]
    
    if not openai_success or not shard_success:
        return ComparisonSummary(
            openai_avg_ttft_ms=0,
            shard_avg_ttft_ms=0,
            openai_avg_latency_ms=0,
            shard_avg_latency_ms=0,
            openai_avg_tps=0,
            shard_avg_tps=0,
            speedup_factor=0,
            cost_savings_percent=0,
        )
    
    openai_ttft = sum(r.time_to_first_token_ms for r in openai_success) / len(openai_success)
    shard_ttft = sum(r.time_to_first_token_ms for r in shard_success) / len(shard_success)
    openai_latency = sum(r.total_latency_ms for r in openai_success) / len(openai_success)
    shard_latency = sum(r.total_latency_ms for r in shard_success) / len(shard_success)
    openai_tps = sum(r.tokens_per_second for r in openai_success) / len(openai_success)
    shard_tps = sum(r.tokens_per_second for r in shard_success) / len(shard_success)
    
    speedup = openai_latency / shard_latency if shard_latency > 0 else 0
    
    # Estimate cost: OpenAI ~$0.002/1K tokens (mini), Shard = $0 (your hardware)
    openai_tokens = sum(r.tokens_generated for r in openai_success)
    estimated_openai_cost = (openai_tokens / 1000) * 0.002
    cost_savings = 100.0  # Shard is free
    
    return ComparisonSummary(
        openai_avg_ttft_ms=openai_ttft,
        shard_avg_ttft_ms=shard_ttft,
        openai_avg_latency_ms=openai_latency,
        shard_avg_latency_ms=shard_latency,
        openai_avg_tps=openai_tps,
        shard_avg_tps=shard_tps,
        speedup_factor=speedup,
        cost_savings_percent=cost_savings,
    )


def print_results(summary: ComparisonSummary, openai_results: list, shard_results: list):
    """Print benchmark results in a table format."""
    print("\n" + "=" * 70)
    print("                    SHARD BENCHMARK RESULTS")
    print("=" * 70)
    
    print("\n┌─────────────────────────────────────────────────────────────────────┐")
    print("│                        SUMMARY COMPARISON                          │")
    print("├─────────────────────────────────────────────────────────────────────┤")
    print(f"│ {'Metric':<25} {'OpenAI':<20} {'Shard':<20} │")
    print("├─────────────────────────────────────────────────────────────────────┤")
    print(f"│ {'Avg TTFT (ms)':<25} {summary.openai_avg_ttft_ms:<20.1f} {summary.shard_avg_ttft_ms:<20.1f} │")
    print(f"│ {'Avg Latency (ms)':<25} {summary.openai_avg_latency_ms:<20.1f} {summary.shard_avg_latency_ms:<20.1f} │")
    print(f"│ {'Avg Tokens/sec':<25} {summary.openai_avg_tps:<20.1f} {summary.shard_avg_tps:<20.1f} │")
    print("└─────────────────────────────────────────────────────────────────────┘")
    
    print(f"\n🚀 Speedup Factor: {summary.speedup_factor:.2f}x faster with Shard")
    print(f"💰 Estimated Cost Savings: {summary.cost_savings_percent:.0f}% (OpenAI: $0.002/1K tokens, Shard: free)")
    
    print("\n" + "-" * 70)
    print("                        DETAILED RESULTS")
    print("-" * 70)
    
    print("\nOpenAI Results:")
    for i, r in enumerate(openai_results, 1):
        status = "✓" if r.success else "✗"
        print(f"  {i}. [{status}] TTFT: {r.time_to_first_token_ms:>6.1f}ms | "
              f"Latency: {r.total_latency_ms:>6.1f}ms | Tokens: {r.tokens_generated:>3}")
        if r.error:
            print(f"      Error: {r.error}")
    
    print("\nShard Results:")
    for i, r in enumerate(shard_results, 1):
        status = "✓" if r.success else "✗"
        print(f"  {i}. [{status}] TTFT: {r.time_to_first_token_ms:>6.1f}ms | "
              f"Latency: {r.total_latency_ms:>6.1f}ms | Tokens: {r.tokens_generated:>3}")
        if r.error:
            print(f"      Error: {r.error}")


def save_results(summary: ComparisonSummary, openai_results: list, shard_results: list, output_file: str):
    """Save results to JSON file."""
    data = {
        "timestamp": datetime.now().isoformat(),
        "summary": asdict(summary),
        "openai_results": [asdict(r) for r in openai_results],
        "shard_results": [asdict(r) for r in shard_results],
    }
    
    with open(output_file, "w") as f:
        json.dump(data, f, indent=2)
    
    print(f"\n📁 Results saved to: {output_file}")


def main():
    parser = argparse.ArgumentParser(description="Shard Benchmark Comparison")
    parser.add_argument("--openai-key", "-k", help="OpenAI API key (or set OPENAI_API_KEY env)")
    parser.add_argument("--openai-model", "-m", default="gpt-4o-mini", help="OpenAI model to use")
    parser.add_argument("--shard-url", "-s", default="http://localhost:9091", help="Shard daemon URL")
    parser.add_argument("--output", "-o", default="benchmarks/results_latest.json", help="Output file")
    parser.add_argument("--skip-openai", action="store_true", help="Skip OpenAI benchmarks")
    args = parser.parse_args()
    
    api_key = args.openai_key or os.environ.get("OPENAI_API_KEY")
    
    if not api_key and not args.skip_openai:
        print("⚠️  No OpenAI API key provided. Use --skip-openai to run Shard-only benchmarks.")
        print("    Set OPENAI_API_KEY environment variable or use --openai-key")
        sys.exit(1)
    
    print("🧪 Shard Benchmark Comparison")
    print(f"   Test prompts: {len(TEST_PROMPTS)}")
    print(f"   Shard URL: {args.shard_url}")
    if not args.skip_openai:
        print(f"   OpenAI Model: {args.openai_model}")
    
    openai_results = []
    shard_results = []
    
    # Run Shard benchmarks
    print("\n📊 Running Shard benchmarks...")
    for i, test in enumerate(TEST_PROMPTS, 1):
        print(f"   [{i}/{len(TEST_PROMPTS)}] {test['category']}...", end=" ", flush=True)
        result = call_shard(test["prompt"], args.shard_url)
        shard_results.append(result)
        print("✓" if result.success else "✗")
    
    # Run OpenAI benchmarks
    if not args.skip_openai:
        print("\n📊 Running OpenAI benchmarks...")
        for i, test in enumerate(TEST_PROMPTS, 1):
            print(f"   [{i}/{len(TEST_PROMPTS)}] {test['category']}...", end=" ", flush=True)
            result = call_openai(test["prompt"], api_key, args.openai_model)
            openai_results.append(result)
            print("✓" if result.success else "✗")
    
    # Calculate and display results
    summary = calculate_summary(openai_results, shard_results)
    print_results(summary, openai_results, shard_results)
    
    # Save to file
    os.makedirs(os.path.dirname(args.output) or ".", exist_ok=True)
    save_results(summary, openai_results, shard_results, args.output)


if __name__ == "__main__":
    main()
