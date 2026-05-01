## Configuration Recommendations for JWT Use Cases

### Short-lived JWTs (< 15 minutes)
```bash
# 15 minute TTL, 3 minute buckets
TTLBLOOM.CREATE jwt:blacklist 900 180 0.01 5000
```

### Standard JWTs (1 hour)
```bash
# 1 hour TTL, 15 minute buckets
TTLBLOOM.CREATE jwt:blacklist 3600 900 0.01 10000
```

### Long-lived JWTs (24 hours)
```bash
# 24 hour TTL, 1 hour buckets
TTLBLOOM.CREATE jwt:blacklist 86400 3600 0.01 50000
```

### Refresh Tokens (30 days)
```bash
# 30 day TTL, 6 hour buckets
TTLBLOOM.CREATE refresh:blacklist 2592000 21600 0.001 100000
```