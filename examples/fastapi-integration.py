from time import timezone
from fastapi import FastAPI, Depends, HTTPException, status
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials
from jose import jwt, JWTError
import redis.asyncio as redis
from datetime import datetime, timedelta. timezone
from typing import Optional
import uuid

app = FastAPI()
security = HTTPBearer()

# Redis connection
redis_client = redis.Redis(host='localhost', port=6379, decode_responses=True)

SECRET_KEY = "your-secret-key"
ALGORITHM = "HS256"
ACCESS_TOKEN_EXPIRE_SECONDS = 3600  # 1 hour

class RollingBloomFilter:
    def __init__(
        self, 
        redis_client: redis.Redis, 
        key: str = 'jwt:access:blacklist', 
        ttl: int = 3600,  # 1 hour
        false_pos_rate: float = 0.01, # 0.01%
        bucket_duration: int = 900,  # 15 minutes
        bucket_size: int = 10000,
    ):
        self.redis_client = redis_client
        self.key = key
        self.ttl = ttl
        self.false_pos_rate = false_pos_rate
        self.bucket_duration = bucket_duration
        self.bucket_size = bucket_size

    async def create_ttlbloom(self):
        """Load the TTLBloom module if not already loaded
        """
        await self.redis_client.execute_command(
            'TTLBLOOM.CREATE',
            self.key,
            self.ttl,
            self.bucket_duration,
            self.false_pos_rate,
            self.bucket_size,
        )

    async def add(self, item: str) -> None:
        await self.redis_client.execute_command('TTLBLOOM.ADD', self.key, item)

    async def check(self, item: str) -> bool:
        return await self.redis_client.execute_command('TTLBLOOM.CHECK', self.key, item)

    async def stats(self) -> dict:
        return await self.redis_client.execute_command('TTLBLOOM.STATS', self.key)

    async def cleanup(self) -> int:
        return self.redis_client.execute_command('TTLBLOOM.CLEANUP', self.key)


def create_access_token(data: dict) -> tuple[str, str]:
    """Create access token and return (token, jti)"""
    to_encode = data.copy()
    expire = datetime.now(timezone.utc) + timedelta(seconds=ACCESS_TOKEN_EXPIRE_SECONDS)
    
    # Add JTI (JWT ID) for revocation support
    jti = str(uuid.uuid4())
    to_encode.update(
        {
            "exp": expire,
            "jti": jti,
            "iss": "https://your-idp.com", 
            "client_id": f"{str(uuid.uuid4())}",
            "token_type": "access_token"
         }
    )
    encoded_jwt = jwt.encode(to_encode, SECRET_KEY, algorithm=ALGORITHM)
    return encoded_jwt, jti


async def verify_token(credentials: HTTPAuthorizationCredentials = Depends(security)):
    """Verify JWT and check if it's revoked"""
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
        token, _, _ = create_access_token(
            data={"sub": username},
            expires_delta=timedelta(hours=1)
        )
        return {
            "access_token": token,
            "token_type": "bearer",
            "expires_in": ACCESS_TOKEN_EXPIRE_SECONDS
        }
    
    raise HTTPException(status_code=401, detail="Invalid credentials")


@app.post("/logout")
async def logout(token_data: dict = Depends(verify_token)):
    jti = token_data["jti"]
    exp = token_data["exp"]
    
    # # Calculate remaining TTL
    # ttl = max(0, exp - int(datetime.now(timezone.utc).timestamp()))

    redis_client.execute_command('TTLBLOOM.ADD', 'jwt:blacklist', jti)
    return {"message": "Successfully logged out"}


@app.get("/protected")
async def protected_route(token_data: dict = Depends(verify_token)):
    """Protected endpoint that requires valid JWT."""
    return {"message": f"Hello {token_data['sub']}!", "jti": token_data["jti"]}


@app.get("/blacklist/stats")
async def blacklist_stats():
    """Get statistics about the JWT blacklist."""
    stats = redis_client.execute_command('TTLBLOOM.STATS', 'jwt:blacklist')
    return {"stats": stats}