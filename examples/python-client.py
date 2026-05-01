import redis
import uuid


# Connect to Redis
r = redis.Redis(host="localhost", port=6379, decode_responses=True)

# Create a TTL Bloom Filter for JWT blacklist
# All JWTs added will have a 3600 second (1 hour) TTL
r.execute_command(
    "TTLBLOOM.CREATE",
    "jwt:blacklist",
    3600,  # default_ttl_secs (all items expire after 1 hour)
    900,  # bucket_duration_secs (15 minute buckets)
    0.01,  # false_positive_rate (1%)
    10000,  # expected_items_per_bucket
)

# Revoke a JWT (add its JTI to blacklist)
# No need to specify TTL - uses the default from CREATE
jti = str(uuid.uuid4())
result = r.execute_command("TTLBLOOM.ADD", "jwt:blacklist", jti)
print(f"Added JTI {jti}: {result}")

# Check if a JWT is revoked
is_revoked = r.execute_command("TTLBLOOM.CHECK", "jwt:blacklist", jti)
print(f"Is JWT revoked? {bool(is_revoked)}")

# Get statistics
stats = r.execute_command("TTLBLOOM.STATS", "jwt:blacklist")
print(f"Blacklist stats:\n{stats}")
