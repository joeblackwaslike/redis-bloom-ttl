#!/usr/bin/env bash

# Create filter with 1 hour default TTL, 15 minute buckets
redis-cli TTLBLOOM.CREATE jwt:blacklist 3600 900 0.01 10000

# Add JTI to blacklist (no TTL argument needed)
redis-cli TTLBLOOM.ADD jwt:blacklist "550e8400-e29b-41d4-a716-446655440000"

# Check if JTI is blacklisted
redis-cli TTLBLOOM.CHECK jwt:blacklist "550e8400-e29b-41d4-a716-446655440000"

# Get statistics
redis-cli TTLBLOOM.STATS jwt:blacklist

# Manual cleanup
redis-cli TTLBLOOM.CLEANUP jwt:blacklist