# Shard Cloud Architecture

## Overview

Shard Cloud is a hosted tier for enterprises that want the cost savings of distributed inference without managing their own verifier nodes.

## Architecture Design

### How Shard Cloud Differs from Self-Hosted

| Aspect | Self-Hosted | Shard Cloud |
|--------|-------------|-------------|
| **Verifier Nodes** | Customer runs their own | Shard-managed fleet |
| **Scout Pool** | Customer's users | Shard public network + customer's users |
| **Billing** | Free (self-managed) | Per verified token |
| **Setup Time** | 10 minutes | Instant (API key only) |
| **Support** | Community | Dedicated |

### Who Runs Verifier Nodes

- Shard maintains a fleet of GPU-enabled verifier nodes
- Nodes are geographically distributed for low latency
- Enterprise traffic is isolated from public traffic
- Capacity scales automatically based on demand

### Traffic Isolation

Enterprise traffic uses dedicated routing:
1. Customer receives an API key tied to their organization
2. All requests include `X-Shard-API-Key` header
3. Traffic is routed to dedicated or isolated verifier pools
4. Usage is metered per-customer

### Billing Model

**Per Verified Token:**
- Base rate: $0.0001 per verified token
- Draft tokens are free (they come from Scouts)
- Example: 100 tokens generated, 80% acceptance = 80 verified tokens = $0.008

**Enterprise Tier:**
- Monthly commitment: $500+
- Discounted rate: $0.00007 per verified token
- Dedicated capacity guarantee

---

## API Key Management

### Key Types

| Tier | Rate Limit | Features |
|------|------------|----------|
| **Free** | 60 req/min | Basic access, public Scout pool |
| **Enterprise** | 600 req/min | Dedicated capacity, priority routing, SLA |

### Key Format

```
sk-shard_prod_<32-char-random>
```

### Management Endpoints

```
POST /v1/admin/api-keys     - Create API key (admin only)
GET  /v1/admin/api-keys      - List API keys (admin only)
DELETE /v1/admin/api-keys/:key - Revoke API key (admin only)
```

### Authentication Flow

1. Customer signs up → receives API key
2. Request includes `X-Shard-API-Key: sk-shard_prod_...`
3. Daemon validates key against auth service
4. If valid: route to verifier, meter usage
5. If invalid: return 401

---

## API Gateway Rate Limiting

### Token Bucket Algorithm

The rate limiter uses a token bucket algorithm with the following parameters:

| Parameter | Default | Enterprise |
|-----------|---------|------------|
| **Requests per minute** | 60 | 600 |
| **Burst allowance** | 10 | 50 |
| **Per-IP limit (unauthenticated)** | 10 req/min | 10 req/min |

### Implementation

**In-Memory Token Bucket (Single Node):**
- Each API key has a bucket with `capacity` tokens
- Each request consumes 1 token
- Tokens refill at `rate` per second
- When bucket is empty: return HTTP 429

**Response Headers on Rate Limit:**
```
Retry-After: <seconds until bucket refills>
X-RateLimit-Limit: 60
X-RateLimit-Remaining: 0
X-RateLimit-Reset: <unix timestamp>
```

### Configuration

```bash
# Rate limiting configuration
SHARD_RATE_LIMIT_DEFAULT=60        # Default tier requests/minute
SHARD_RATE_LIMIT_BURST=10          # Burst allowance
SHARD_RATE_LIMIT_ENTERPRISE=600    # Enterprise tier requests/minute
```

### Queue Management

- **Max pending drafts**: 500
- When exceeded: return HTTP 503 with `Retry-After` header
- Prevents verifier resource exhaustion

---

## Usage Metering

### Data Schema

```sql
CREATE TABLE usage_records (
    id BIGSERIAL PRIMARY KEY,
    api_key VARCHAR(64) NOT NULL,
    timestamp TIMESTAMP NOT NULL DEFAULT NOW(),
    tokens_verified INTEGER NOT NULL,
    tokens_accepted INTEGER NOT NULL,
    request_id VARCHAR(64),
    latency_ms INTEGER
);

CREATE INDEX idx_usage_api_key_time ON usage_records(api_key, timestamp);
```

### Endpoints

```
GET /v1/usage              - Current period usage (caller's key)
GET /v1/usage/rate-limit   - Current rate limit status
GET /v1/admin/usage/:key   - Usage for specific key (admin only)
```

### Response Format

```json
{
  "period": "2026-02-22",
  "tokens_verified": 125000,
  "tokens_accepted": 100000,
  "acceptance_rate": 0.80,
  "requests": 1500,
  "estimated_cost": "$12.50"
}
```

---

## Deployment Architecture

```
                    ┌─────────────────────┐
                    │   Load Balancer     │
                    │   (Cloudflare/AWS)  │
                    └─────────┬───────────┘
                              │
              ┌───────────────┼───────────────┐
              │               │               │
              ▼               ▼               ▼
        ┌─────────┐    ┌─────────┐    ┌─────────┐
        │ Gateway │    │ Gateway │    │ Gateway │
        │ Node 1  │    │ Node 2  │    │ Node 3  │
        └────┬────┘    └────┬────┘    └────┬────┘
             │              │              │
             └──────────────┼──────────────┘
                            │
                    ┌───────┴───────┐
                    │  Redis Cluster │
                    │  (rate limit   │
                    │   + metering)  │
                    └───────┬───────┘
                            │
         ┌──────────────────┼──────────────────┐
         │                  │                  │
         ▼                  ▼                  ▼
   ┌──────────┐      ┌──────────┐      ┌──────────┐
   │ Verifier │      │ Verifier │      │ Verifier │
   │ Pool A   │      │ Pool B   │      │ Pool C   │
   │ (Ent A)  │      │ (Public) │      │ (Ent B)  │
   └──────────┘      └──────────┘      └──────────┘
```

---

## Security Considerations

### API Key Security
- Keys are hashed before storage (SHA-256)
- Keys are only shown once at creation time
- Keys can be rotated via admin API

### Network Security
- All traffic over TLS 1.3
- Private mesh mode available for enterprises
- DDoS protection via Cloudflare

### Rate Limiting Goals
- Prevent single API key from monopolizing verifier capacity
- Protect against credential stuffing attacks
- Graceful degradation under attack

---

## Migration Path

### From Self-Hosted to Cloud

1. Sign up for Shard Cloud
2. Receive API key
3. Update client configuration:
   ```python
   client = Shard(
       base_url="https://api.shardcloud.io/v1",
       api_key="sk-shard_prod_..."
   )
   ```
4. Monitor usage at `/v1/usage`
5. Decommission self-hosted nodes (optional)

### From Cloud to Self-Hosted

1. Export usage data for billing
2. Download Shard daemon
3. Run with `SHARD_API_KEYS=your_key`
4. Traffic shifts automatically (client-side config change)
