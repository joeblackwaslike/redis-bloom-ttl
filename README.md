# Production-Ready Redis Bloom Filter Module with Individual TTL

[![Discord](https://img.shields.io/discord/1486035859747897414?logo=discord&label=Discord&color=5865F2)](https://discord.com/channels/1486035859747897414/1509515337417097277) [![Join Discord](https://img.shields.io/badge/Discord-Join%20Server-5865F2?logo=discord&logoColor=white)](https://discord.gg/Fjc9zYHZyV)


This is a sophisticated implementation perfect for JWT blacklisting. Since standard bloom filters don't support deletion (required for TTL), I'll implement a **time-partitioned bloom filter** approach that organizes items into buckets based on expiration time.

## Architecture Overview
```
┌─────────────────────────────────────────┐
│   TTL Bloom Filter (Redis Key)          │
├─────────────────────────────────────────┤
│  Metadata:                              │
│  - bucket_duration_secs                 │
│  - false_positive_rate                  │
│  - expected_items_per_bucket            │
├─────────────────────────────────────────┤
│  Time Buckets (sorted by expiration):  │
│  ┌──────────────────────────────────┐  │
│  │ Bucket 1735689600 (expires 2025)│  │
│  │ - bloom filter data              │  │
│  └──────────────────────────────────┘  │
│  ┌──────────────────────────────────┐  │
│  │ Bucket 1735693200 (expires 2025)│  │
│  │ - bloom filter data              │  │
│  └──────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

## Implementation

### Project Structure

### 1. Build the Module

```bash
cargo build --release
```

The compiled module will be at: `target/release/libredis_ttl_bloom.dylib` (macOS) or `.so` (Linux)

### 2. Load into Redis

**Option A: Load at runtime**
```bash
redis-server --loadmodule ./target/release/libredis_ttl_bloom.so
```

**Option B: Add to redis.conf**
```
loadmodule /path/to/libredis_ttl_bloom.so
```

### 3. Verify Installation
```bash
redis-cli
127.0.0.1:6379> MODULE LIST
```

## Usage Examples

### JWT Blacklist Use Case

```python
import redis
import uuid
from datetime import datetime, timedelta

# Connect to Redis
r = redis.Redis(host='localhost', port=6379, decode_responses=True)

# Create a TTL Bloom Filter for JWT blacklist
# - Bucket duration: 3600 seconds (1 hour buckets)
# - False positive rate: 0.01 (1%)
# - Expected items per bucket: 10000 JWTs
r.execute_command(
    'TTLBLOOM.CREATE',
    'jwt:blacklist',
    3600,      # bucket_duration_secs
    0.01,      # false_positive_rate
    10000      # expected_items_per_bucket
)

# Revoke a JWT (add its JTI to blacklist)
jti = str(uuid.uuid4())
ttl_seconds = 3600  # JWT expires in 1 hour

result = r.execute_command('TTLBLOOM.ADD', 'jwt:blacklist', jti, ttl_seconds)
print(f"Added JTI {jti}: {result}")

# Check if a JWT is revoked
is_revoked = r.execute_command('TTLBLOOM.CHECK', 'jwt:blacklist', jti)
print(f"Is JWT revoked? {bool(is_revoked)}")

# Get statistics
stats = r.execute_command('TTLBLOOM.STATS', 'jwt:blacklist')
print(f"Blacklist stats:\n{stats}")

# Manual cleanup (optional - happens automatically on check/add)
removed = r.execute_command('TTLBLOOM.CLEANUP', 'jwt:blacklist')
print(f"Removed {removed} expired buckets")
```

### FastAPI Integration Example

```python
from fastapi import FastAPI, Depends, HTTPException, status
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials
from jose import jwt, JWTError
import redis
from datetime import datetime, timedelta
from typing import Optional
import uuid

app = FastAPI()
security = HTTPBearer()

# Redis connection
redis_client = redis.Redis(host='localhost', port=6379, decode_responses=True)

# Initialize bloom filter
redis_client.execute_command(
    'TTLBLOOM.CREATE', 'jwt:blacklist', 3600, 0.01, 10000
)

SECRET_KEY = "your-secret-key"
ALGORITHM = "HS256"

def create_access_token(data: dict, expires_delta: Optional[timedelta] = None):
    to_encode = data.copy()
    expire = datetime.utcnow() + (expires_delta or timedelta(hours=1))
    
    # Add JTI (JWT ID) for revocation support
    jti = str(uuid.uuid4())
    to_encode.update({"exp": expire, "jti": jti})
    
    encoded_jwt = jwt.encode(to_encode, SECRET_KEY, algorithm=ALGORITHM)
    return encoded_jwt, jti, int(expires_delta.total_seconds()) if expires_delta else 3600

async def verify_token(credentials: HTTPAuthorizationCredentials = Depends(security)):
    try:
        payload = jwt.decode(
            credentials.credentials,
            SECRET_KEY,
            algorithms=[ALGORITHM]
        )
        
        jti: str = payload.get("jti")
        if jti is None:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Token missing JTI"
            )
        
        # Check if token is revoked (in blacklist)
        is_revoked = redis_client.execute_command(
            'TTLBLOOM.CHECK', 'jwt:blacklist', jti
        )
        
        if is_revoked:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Token has been revoked"
            )
        
        return payload
        
    except JWTError:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Could not validate credentials"
        )

@app.post("/login")
async def login(username: str, password: str):
    # Your authentication logic here
    if username == "test" and password == "test":
        token, jti, ttl = create_access_token(
            data={"sub": username},
            expires_delta=timedelta(hours=1)
        )
        return {"access_token": token, "token_type": "bearer"}
    
    raise HTTPException(status_code=401, detail="Invalid credentials")

@app.post("/logout")
async def logout(token_data: dict = Depends(verify_token)):
    jti = token_data["jti"]
    exp = token_data["exp"]
    
    # Calculate remaining TTL
    ttl = max(0, exp - int(datetime.utcnow().timestamp()))
    
    # Add to blacklist with remaining TTL
    redis_client.execute_command('TTLBLOOM.ADD', 'jwt:blacklist', jti, ttl)
    
    return {"message": "Successfully logged out"}

@app.get("/protected")
async def protected_route(token_data: dict = Depends(verify_token)):
    return {"message": f"Hello {token_data['sub']}!"}

@app.get("/blacklist/stats")
async def blacklist_stats():
    stats = redis_client.execute_command('TTLBLOOM.STATS', 'jwt:blacklist')
    return {"stats": stats}
```

## Production Considerations
### 1. **Memory Management**
- Each bucket uses approximately: `(num_bits / 8) + overhead` bytes
- Monitor with `TTLBLOOM.STATS`
- Consider bucket duration vs memory tradeoff

### 2. **False Positive Rate**
- Start with 0.01 (1%) for JWT blacklists
- Lower rates = more memory but fewer false positives
- Monitor actual FP rate in stats

### 3. **Bucket Duration Strategy**
For JWTs:
- Short-lived tokens (< 1 hour): 300-600 second buckets
- Medium tokens (1-24 hours): 1800-3600 second buckets
- Long-lived tokens: 3600-7200 second buckets


## Next Steps
1. **Performance Testing**: Benchmark with realistic JWT loads
2. **Optimization**: Consider using [probabilistic data structures crate](https://crates.io/crates/probabilistic-collections)
3. **Monitoring**: Add Prometheus metrics
4. **Documentation**: Add rustdoc comments
5. **CI/CD**: Set up automated testing and builds

Would you like me to elaborate on any specific part or add additional features like bulk operations, export/import, or advanced monitoring?