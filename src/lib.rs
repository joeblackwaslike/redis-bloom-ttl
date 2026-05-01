use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use redis_module::{redis_module, Context, RedisError, RedisResult, RedisString, RedisValue};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// CUSTOM SERIALIZABLE BLOOM FILTER
// ============================================================================

/// A simple, serializable Bloom filter implementation
#[derive(Serialize, Deserialize, Clone)]
pub struct SerializableBloomFilter {
    /// Bit array for the bloom filter
    pub bit_array: Vec<u64>,
    /// Number of hash functions
    pub num_hashes: u32,
    /// Number of bits in the filter
    pub num_bits: usize,
    /// Number of items added
    pub item_count: usize,
}

impl SerializableBloomFilter {
    /// Create a new bloom filter optimized for given parameters
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        // Calculate optimal number of bits
        let num_bits = Self::optimal_num_bits(expected_items, false_positive_rate);
        // Calculate optimal number of hash functions
        let num_hashes = Self::optimal_num_hashes(expected_items, num_bits);

        // Calculate number of u64s needed
        let array_size = (num_bits + 63) / 64;

        Self {
            bit_array: vec![0u64; array_size],
            num_hashes,
            num_bits,
            item_count: 0,
        }
    }

    /// Calculate optimal number of bits for given parameters
    /// Formula: m = -n * ln(p) / (ln(2)^2)
    pub fn optimal_num_bits(n: usize, p: f64) -> usize {
        let ln2_squared = std::f64::consts::LN_2 * std::f64::consts::LN_2;
        (-(n as f64) * p.ln() / ln2_squared).ceil() as usize
    }

    /// Calculate optimal number of hash functions
    /// Formula: k = (m/n) * ln(2)
    pub fn optimal_num_hashes(n: usize, m: usize) -> u32 {
        ((m as f64 / n as f64) * std::f64::consts::LN_2).ceil() as u32
    }

    /// Generate hash values for an item
    pub fn hash_values(&self, item: &str) -> Vec<usize> {
        let mut hashes = Vec::with_capacity(self.num_hashes as usize);

        for i in 0..self.num_hashes {
            let mut hasher = DefaultHasher::new();
            item.hash(&mut hasher);
            i.hash(&mut hasher);
            let hash = hasher.finish();
            hashes.push((hash as usize) % self.num_bits);
        }

        hashes
    }

    /// Set a bit in the bit array
    pub fn set_bit(&mut self, index: usize) {
        let array_index = index / 64;
        let bit_index = index % 64;
        self.bit_array[array_index] |= 1u64 << bit_index;
    }

    /// Check if a bit is set
    pub fn get_bit(&self, index: usize) -> bool {
        let array_index = index / 64;
        let bit_index = index % 64;
        (self.bit_array[array_index] & (1u64 << bit_index)) != 0
    }

    /// Add an item to the bloom filter
    pub fn add(&mut self, item: &str) {
        let hashes = self.hash_values(item);
        for hash in hashes {
            self.set_bit(hash);
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the bloom filter
    pub fn contains(&self, item: &str) -> bool {
        let hashes = self.hash_values(item);
        hashes.iter().all(|&hash| self.get_bit(hash))
    }

    /// Calculate current false positive rate
    pub fn current_false_positive_rate(&self) -> f64 {
        if self.item_count == 0 {
            return 0.0;
        }

        let k = self.num_hashes as f64;
        let m = self.num_bits as f64;
        let n = self.item_count as f64;

        (1.0 - (-k * n / m).exp()).powf(k)
    }
}

// ============================================================================
// TTL BLOOM FILTER
// ============================================================================

/// Metadata for the TTL Bloom Filter
#[derive(Serialize, Deserialize, Clone)]
pub struct TTLBloomMetadata {
    /// Default TTL in seconds for all items added to this filter
    pub default_ttl_secs: u64,
    /// Duration of each time bucket in seconds
    pub bucket_duration_secs: u64,
    /// Target false positive rate
    pub false_positive_rate: f64,
    /// Expected number of items per bucket
    pub expected_items_per_bucket: usize,
}

/// A time-partitioned bloom filter bucket
#[derive(Serialize, Deserialize, Clone)]
pub struct BloomBucket {
    /// Unix timestamp when this bucket expires
    pub expires_at: u64,
    /// The bloom filter for this bucket
    pub filter: SerializableBloomFilter,
}

/// Main TTL Bloom Filter structure
#[derive(Serialize, Deserialize, Clone)]
pub struct TTLBloomFilter {
    pub metadata: TTLBloomMetadata,
    /// Buckets organized by expiration timestamp
    pub buckets: BTreeMap<u64, BloomBucket>,
}

impl TTLBloomFilter {
    /// Create a new TTL Bloom Filter
    pub fn new(
        default_ttl_secs: u64,
        bucket_duration_secs: u64,
        false_positive_rate: f64,
        expected_items_per_bucket: usize,
    ) -> Self {
        Self {
            metadata: TTLBloomMetadata {
                default_ttl_secs,
                bucket_duration_secs,
                false_positive_rate,
                expected_items_per_bucket,
            },
            buckets: BTreeMap::new(),
        }
    }

    /// Get current Unix timestamp
    pub fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Calculate which bucket an item should go into based on default TTL
    pub fn calculate_bucket_timestamp(&self) -> u64 {
        let current_time = Self::current_timestamp();
        let expires_at = current_time + self.metadata.default_ttl_secs;

        // Round up to nearest bucket boundary
        let bucket_start = ((expires_at + self.metadata.bucket_duration_secs - 1)
            / self.metadata.bucket_duration_secs)
            * self.metadata.bucket_duration_secs;

        bucket_start
    }

    /// Remove expired buckets
    pub fn cleanup_expired(&mut self) -> usize {
        let current_time = Self::current_timestamp();
        let expired_keys: Vec<u64> = self
            .buckets
            .range(..current_time)
            .map(|(k, _)| *k)
            .collect();

        let count = expired_keys.len();
        for key in expired_keys {
            self.buckets.remove(&key);
        }
        count
    }

    /// Add an item using the default TTL
    pub fn add(&mut self, item: &str) -> bool {
        // Cleanup expired buckets first
        self.cleanup_expired();

        let bucket_timestamp = self.calculate_bucket_timestamp();

        // Get or create the bucket
        let bucket = self.buckets.entry(bucket_timestamp).or_insert_with(|| {
            let filter = SerializableBloomFilter::new(
                self.metadata.expected_items_per_bucket,
                self.metadata.false_positive_rate,
            );

            BloomBucket {
                expires_at: bucket_timestamp,
                filter,
            }
        });

        // Add item to the bucket's bloom filter
        bucket.filter.add(item);
        true
    }

    /// Check if an item exists (might be expired)
    pub fn check(&mut self, item: &str) -> bool {
        // Cleanup expired buckets first
        self.cleanup_expired();

        // Check all remaining buckets
        for bucket in self.buckets.values() {
            if bucket.filter.contains(item) {
                return true;
            }
        }

        false
    }

    /// Get statistics about the filter
    pub fn stats(&mut self) -> FilterStats {
        self.cleanup_expired();

        let total_items: usize = self.buckets.values().map(|b| b.filter.item_count).sum();
        let total_buckets = self.buckets.len();
        let current_time = Self::current_timestamp();

        let bucket_info: Vec<BucketInfo> = self
            .buckets
            .iter()
            .map(|(ts, bucket)| BucketInfo {
                expires_at: *ts,
                expires_in_secs: ts.saturating_sub(current_time),
                item_count: bucket.filter.item_count,
                estimated_fp_rate: bucket.filter.current_false_positive_rate(),
                num_bits: bucket.filter.num_bits,
                num_hashes: bucket.filter.num_hashes,
            })
            .collect();

        FilterStats {
            total_items,
            total_buckets,
            default_ttl_secs: self.metadata.default_ttl_secs,
            bucket_duration_secs: self.metadata.bucket_duration_secs,
            false_positive_rate: self.metadata.false_positive_rate,
            buckets: bucket_info,
        }
    }

    /// Serialize to base64-encoded string
    pub fn to_base64(&self) -> Result<String, String> {
        let json_bytes =
            serde_json::to_vec(self).map_err(|e| format!("Serialization error: {}", e))?;
        Ok(BASE64.encode(&json_bytes))
    }

    /// Deserialize from base64-encoded string
    pub fn from_base64(encoded: &str) -> Result<Self, String> {
        let json_bytes = BASE64
            .decode(encoded)
            .map_err(|e| format!("Base64 decode error: {}", e))?;
        serde_json::from_slice(&json_bytes).map_err(|e| format!("Deserialization error: {}", e))
    }
}

#[derive(Serialize)]
pub struct FilterStats {
    pub total_items: usize,
    pub total_buckets: usize,
    pub default_ttl_secs: u64,
    pub bucket_duration_secs: u64,
    pub false_positive_rate: f64,
    pub buckets: Vec<BucketInfo>,
}

#[derive(Serialize)]
pub struct BucketInfo {
    expires_at: u64,
    expires_in_secs: u64,
    item_count: usize,
    estimated_fp_rate: f64,
    num_bits: usize,
    num_hashes: u32,
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Read a string value from a Redis key
fn read_string_key(ctx: &Context, key: &RedisString) -> RedisResult<Option<String>> {
    let redis_key = ctx.open_key(key);

    match redis_key.read()? {
        Some(value) => {
            // Convert &[u8] to String
            let s = std::str::from_utf8(value)
                .map_err(|e| RedisError::String(format!("UTF-8 decode error: {}", e)))?;
            Ok(Some(s.to_string()))
        }
        None => Ok(None),
    }
}

/// Write a string value to a Redis key
fn write_string_key(ctx: &Context, key: &RedisString, value: &str) -> RedisResult<()> {
    let redis_key = ctx.open_key_writable(key);
    redis_key.write(value)?;
    Ok(())
}

// ============================================================================
// REDIS MODULE COMMANDS
// ============================================================================

/// TTLBLOOM.CREATE key default_ttl_secs bucket_duration_secs false_positive_rate expected_items_per_bucket
fn ttlbloom_create(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() != 6 {
        return Err(RedisError::WrongArity);
    }

    let key = &args[1];
    let default_ttl: u64 = args[2]
        .try_as_str()?
        .parse()
        .map_err(|_| RedisError::String("Invalid default_ttl".into()))?;
    let bucket_duration: u64 = args[3]
        .try_as_str()?
        .parse()
        .map_err(|_| RedisError::String("Invalid bucket_duration".into()))?;
    let fp_rate: f64 = args[4]
        .try_as_str()?
        .parse()
        .map_err(|_| RedisError::String("Invalid false_positive_rate".into()))?;
    let expected_items: usize = args[5]
        .try_as_str()?
        .parse()
        .map_err(|_| RedisError::String("Invalid expected_items".into()))?;

    if fp_rate <= 0.0 || fp_rate >= 1.0 {
        return Err(RedisError::String(
            "false_positive_rate must be between 0 and 1".into(),
        ));
    }

    if bucket_duration == 0 {
        return Err(RedisError::String(
            "bucket_duration must be greater than 0".into(),
        ));
    }

    if default_ttl == 0 {
        return Err(RedisError::String(
            "default_ttl must be greater than 0".into(),
        ));
    }

    let filter = TTLBloomFilter::new(default_ttl, bucket_duration, fp_rate, expected_items);
    let serialized = filter.to_base64().map_err(|e| RedisError::String(e))?;

    write_string_key(ctx, key, &serialized)?;

    Ok(RedisValue::SimpleString("OK".into()))
}

/// TTLBLOOM.ADD key item
fn ttlbloom_add(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() != 3 {
        return Err(RedisError::WrongArity);
    }

    let key = &args[1];
    let item = args[2].try_as_str()?;

    let current_value = read_string_key(ctx, key)?;

    let mut filter: TTLBloomFilter = match current_value {
        Some(data) => TTLBloomFilter::from_base64(&data).map_err(|e| RedisError::String(e))?,
        None => {
            return Err(RedisError::String(
                "Key does not exist. Use TTLBLOOM.CREATE first".into(),
            ))
        }
    };

    filter.add(item);

    let serialized = filter.to_base64().map_err(|e| RedisError::String(e))?;

    write_string_key(ctx, key, &serialized)?;

    Ok(RedisValue::Integer(1))
}

/// TTLBLOOM.CHECK key item
fn ttlbloom_check(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() != 3 {
        return Err(RedisError::WrongArity);
    }

    let key = &args[1];
    let item = args[2].try_as_str()?;

    let current_value = read_string_key(ctx, key)?;

    let mut filter: TTLBloomFilter = match current_value {
        Some(data) => TTLBloomFilter::from_base64(&data).map_err(|e| RedisError::String(e))?,
        None => return Ok(RedisValue::Integer(0)),
    };

    let exists = filter.check(item);

    // Write back to persist cleanup
    let serialized = filter.to_base64().map_err(|e| RedisError::String(e))?;
    write_string_key(ctx, key, &serialized)?;

    Ok(RedisValue::Integer(if exists { 1 } else { 0 }))
}

/// TTLBLOOM.STATS key
fn ttlbloom_stats(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() != 2 {
        return Err(RedisError::WrongArity);
    }

    let key = &args[1];

    let current_value = read_string_key(ctx, key)?;

    let mut filter: TTLBloomFilter = match current_value {
        Some(data) => TTLBloomFilter::from_base64(&data).map_err(|e| RedisError::String(e))?,
        None => return Err(RedisError::String("Key does not exist".into())),
    };

    let stats = filter.stats();

    // Write back to persist cleanup
    let serialized_filter = filter.to_base64().map_err(|e| RedisError::String(e))?;
    write_string_key(ctx, key, &serialized_filter)?;

    let stats_json = serde_json::to_string_pretty(&stats)
        .map_err(|e| RedisError::String(format!("Stats serialization error: {}", e)))?;

    Ok(RedisValue::BulkString(stats_json))
}

/// TTLBLOOM.CLEANUP key
fn ttlbloom_cleanup(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() != 2 {
        return Err(RedisError::WrongArity);
    }

    let key = &args[1];

    let current_value = read_string_key(ctx, key)?;

    let mut filter: TTLBloomFilter = match current_value {
        Some(data) => TTLBloomFilter::from_base64(&data).map_err(|e| RedisError::String(e))?,
        None => return Err(RedisError::String("Key does not exist".into())),
    };

    let removed = filter.cleanup_expired();

    let serialized = filter.to_base64().map_err(|e| RedisError::String(e))?;
    write_string_key(ctx, key, &serialized)?;

    Ok(RedisValue::Integer(removed as i64))
}

// ============================================================================
// MODULE INITIALIZATION
// ============================================================================

redis_module! {
    name: "ttlbloom",
    version: 1,
    allocator: (redis_module::alloc::RedisAlloc, redis_module::alloc::RedisAlloc),
    data_types: [],
    commands: [
        ["ttlbloom.create", ttlbloom_create, "write deny-oom", 1, 1, 1],
        ["ttlbloom.add", ttlbloom_add, "write deny-oom", 1, 1, 1],
        ["ttlbloom.check", ttlbloom_check, "write", 1, 1, 1],
        ["ttlbloom.stats", ttlbloom_stats, "write readonly", 1, 1, 1],
        ["ttlbloom.cleanup", ttlbloom_cleanup, "write", 1, 1, 1],
    ],
}
