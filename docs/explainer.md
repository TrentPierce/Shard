# Shard: Browser-Powered AI Inference

---

## The Problem

Every token your AI application generates costs you money. At scale, centralized GPU compute is the single largest line item for companies building AI products. Whether you're using OpenAI, Anthropic, or self-hosted models — you're renting expensive GPUs that sit idle most of the time.

If you're spending $5,000/month on AI APIs, this document is for you.

---

## The Insight

Your users' browsers are sitting idle right now. Every laptop, tablet, and phone that visits your website has GPU power going to waste. What if that compute could serve your AI traffic?

That's Shard. A distributed inference network that turns your users' devices into AI co-processors.

---

## How Shard Works

**It's surprisingly simple:**

1. **Your users become Scouts.** When someone opens your app, their browser automatically joins the Shard network as a lightweight "Scout" node. It downloads a small draft model (~200MB) that runs on WebGPU.

2. **Scouts draft, Shards verify.** When you need AI output, the browser generates candidate tokens locally. Your server (the "Shard") receives these drafts and verifies them against your full model. Most drafts are accepted. Invalid drafts are resampled.

3. **Users get fast responses.** The result is served from your infrastructure, but with help from your user community. You pay only for verification — not full generation.

**The math works out:** If 10% of your monthly active users contribute Scout compute, you can reduce AI costs by 40-80%.

---

## What You Get

- **OpenAI-Compatible API** — Drop Shard into your existing codebase with a single endpoint change
- **40-80% Cost Reduction** — Trade cloud GPU rentals for your users' idle compute  
- **No Infrastructure to Manage** — Run a single binary. Models download automatically.
- **Self-Healing Network** — If a user closes their browser, traffic routes to other nodes automatically
- **Complete Control** — Private mesh option lets you run Shard on your own servers with your own models

---

## Pilot Offer

We set up a private Shard network for your organization in one week, free.

**You need:**
- 10+ active browser users (employees, customers, community)
- One server (can be a $10/mo VPS to start)

**We provide:**
- Network setup and configuration
- Integration with your existing API calls
- Monitoring dashboard
- 30-day free trial

After the pilot, you decide: scale up, or walk away. No contracts, no lock-in.

---

## Who This Is For

| ✓ | ✗ |
|---|---|
| Companies spending $5K+/month on AI APIs | Teams just exploring AI concepts |
| Apps with 1K+ monthly active users | Low-traffic internal tools |
| Product teams wanting to reduce AI costs | Research projects needing maximal model size |
| Organizations with privacy requirements | Companies prohibited from user compute |

---

## Contact

**Ready to try Shard?**

Email: [your-email@placeholder.com]

GitHub: https://github.com/TrentPierce/Shard

Website: https://shardnetwork.live

---

*Shard is not affiliated with any cryptocurrency project. Credits are an internal accounting mechanism, not a tradeable asset.*
