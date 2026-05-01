//! Unit tests for Redis TTL Bloom Filter module
//! These tests don't require a running Redis server

use redis_ttl_bloom::{SerializableBloomFilter, TTLBloomFilter};

// ============================================================================
// BLOOM FILTER TESTS
// ============================================================================

#[test]
fn test_bloom_filter_creation() {
    let bloom = SerializableBloomFilter::new(1000, 0.01);

    assert_eq!(bloom.item_count, 0);
    assert!(bloom.num_bits > 0);
    assert!(bloom.num_hashes > 0);
    assert!(bloom.bit_array.len() > 0);
}

#[test]
fn test_bloom_filter_optimal_calculations() {
    // Test optimal bits calculation
    let num_bits = SerializableBloomFilter::optimal_num_bits(1000, 0.01);
    // For n=1000, p=0.01, should be around 9585 bits
    assert!(num_bits > 9000 && num_bits < 10000);

    // Test optimal hashes calculation
    let num_hashes = SerializableBloomFilter::optimal_num_hashes(1000, num_bits);
    // Should be around 7 hash functions
    assert!(num_hashes >= 6 && num_hashes <= 8);
}

#[test]
fn test_bloom_filter_add_and_check() {
    let mut bloom = SerializableBloomFilter::new(100, 0.01);

    // Add item
    bloom.add("test-item-1");
    assert_eq!(bloom.item_count, 1);

    // Check it exists
    assert!(bloom.contains("test-item-1"));

    // Check non-existent item (should return false with high probability)
    assert!(!bloom.contains("non-existent-item"));
}

#[test]
fn test_bloom_filter_multiple_items() {
    let mut bloom = SerializableBloomFilter::new(100, 0.01);

    // Add multiple items
    let items = vec!["item1", "item2", "item3", "item4", "item5"];
    for item in &items {
        bloom.add(item);
    }

    assert_eq!(bloom.item_count, 5);

    // All items should be found
    for item in &items {
        assert!(bloom.contains(item), "Item {} should be in filter", item);
    }
}

#[test]
fn test_bloom_filter_false_positive_rate() {
    let mut bloom = SerializableBloomFilter::new(1000, 0.01);

    // Add 1000 items
    for i in 0..1000 {
        bloom.add(&format!("item-{}", i));
    }

    // Test false positive rate
    let mut false_positives = 0;
    let test_count = 1000;

    for i in 1000..1000 + test_count {
        if bloom.contains(&format!("item-{}", i)) {
            false_positives += 1;
        }
    }

    let fp_rate = false_positives as f64 / test_count as f64;

    println!(
        "False positive rate: {:.4} ({}/{})",
        fp_rate, false_positives, test_count
    );

    // Should be close to 0.01, allow up to 3x variance
    assert!(fp_rate < 0.03, "FP rate {} too high", fp_rate);
}

#[test]
fn test_bloom_filter_estimated_fp_rate() {
    let mut bloom = SerializableBloomFilter::new(1000, 0.01);

    // Initially should be 0
    assert_eq!(bloom.current_false_positive_rate(), 0.0);

    // Add items
    for i in 0..500 {
        bloom.add(&format!("item-{}", i));
    }

    // Should be less than target rate at half capacity
    let fp_rate = bloom.current_false_positive_rate();
    println!("FP rate at half capacity: {:.6}", fp_rate);
    assert!(
        fp_rate < 0.01,
        "FP rate {} too high at half capacity",
        fp_rate
    );
}

#[test]
fn test_hash_consistency() {
    let bloom = SerializableBloomFilter::new(1000, 0.01);

    let item = "test-item-123";
    let hashes1 = bloom.hash_values(item);
    let hashes2 = bloom.hash_values(item);

    // Same item should produce same hashes
    assert_eq!(hashes1, hashes2);

    // Should produce correct number of hashes
    assert_eq!(hashes1.len(), bloom.num_hashes as usize);
}

#[test]
fn test_bit_operations() {
    let mut bloom = SerializableBloomFilter::new(100, 0.01);

    // Initially all bits should be 0
    for i in 0..bloom.num_bits {
        assert!(!bloom.get_bit(i));
    }

    // Set some bits
    bloom.set_bit(0);
    bloom.set_bit(10);
    bloom.set_bit(100);

    // Check bits are set
    assert!(bloom.get_bit(0));
    assert!(bloom.get_bit(10));
    assert!(bloom.get_bit(100));

    // Check other bits are still 0
    assert!(!bloom.get_bit(1));
    assert!(!bloom.get_bit(11));
    assert!(!bloom.get_bit(99));
}

#[test]
fn test_large_item_count() {
    let mut bloom = SerializableBloomFilter::new(10000, 0.01);

    // Add 5000 items
    for i in 0..5000 {
        bloom.add(&format!("item-{}", i));
    }

    assert_eq!(bloom.item_count, 5000);

    // Verify random samples
    assert!(bloom.contains("item-0"));
    assert!(bloom.contains("item-2500"));
    assert!(bloom.contains("item-4999"));
}

// ============================================================================
// TTL BLOOM FILTER TESTS
// ============================================================================

#[test]
fn test_ttl_bloom_filter_creation() {
    let filter = TTLBloomFilter::new(
        3600, // 1 hour TTL
        900,  // 15 minute buckets
        0.01, // 1% FP rate
        1000, // 1000 items per bucket
    );

    assert_eq!(filter.metadata.default_ttl_secs, 3600);
    assert_eq!(filter.metadata.bucket_duration_secs, 900);
    assert_eq!(filter.metadata.false_positive_rate, 0.01);
    assert_eq!(filter.metadata.expected_items_per_bucket, 1000);
    assert_eq!(filter.buckets.len(), 0);
}

#[test]
fn test_bucket_timestamp_calculation() {
    let filter = TTLBloomFilter::new(3600, 900, 0.01, 1000);

    let bucket_ts = filter.calculate_bucket_timestamp();

    // Bucket should be aligned to bucket_duration_secs boundary
    assert_eq!(bucket_ts % 900, 0);

    // Should be in the future
    let current_time = TTLBloomFilter::current_timestamp();
    assert!(bucket_ts > current_time);
}

#[test]
fn test_add_item_creates_bucket() {
    let mut filter = TTLBloomFilter::new(3600, 900, 0.01, 1000);

    assert_eq!(filter.buckets.len(), 0);

    filter.add("test-item");

    // Should have created one bucket
    assert_eq!(filter.buckets.len(), 1);

    // Bucket should have one item
    let bucket = filter.buckets.values().next().unwrap();
    assert_eq!(bucket.filter.item_count, 1);
}

#[test]
fn test_check_added_item() {
    let mut filter = TTLBloomFilter::new(3600, 900, 0.01, 1000);

    filter.add("test-jti-12345");

    // Should find the item
    assert!(filter.check("test-jti-12345"));

    // Should not find non-existent item
    assert!(!filter.check("non-existent"));
}

#[test]
fn test_multiple_adds_same_bucket() {
    let mut filter = TTLBloomFilter::new(3600, 900, 0.01, 1000);

    // Add multiple items quickly (should go to same bucket)
    for i in 0..10 {
        filter.add(&format!("item-{}", i));
    }

    // Should still have only one bucket
    assert_eq!(filter.buckets.len(), 1);

    // All items should be found
    for i in 0..10 {
        assert!(filter.check(&format!("item-{}", i)));
    }
}

#[test]
fn test_cleanup_expired_buckets() {
    use std::thread;
    use std::time::Duration;

    // Create filter with very short TTL for testing
    let mut filter = TTLBloomFilter::new(1, 1, 0.01, 1000);

    // Add item
    filter.add("test-item");
    assert_eq!(filter.buckets.len(), 1);

    // Wait for expiration
    thread::sleep(Duration::from_secs(2));

    // Cleanup should remove the bucket
    let removed = filter.cleanup_expired();
    assert!(removed > 0);
    assert_eq!(filter.buckets.len(), 0);
}

#[test]
fn test_check_triggers_cleanup() {
    use std::thread;
    use std::time::Duration;

    // Create filter with very short TTL
    let mut filter = TTLBloomFilter::new(1, 1, 0.01, 1000);

    // Add item
    filter.add("test-item");
    assert!(filter.check("test-item"));

    // Wait for expiration
    thread::sleep(Duration::from_secs(2));

    // Check should trigger cleanup and return false
    assert!(!filter.check("test-item"));
    assert_eq!(filter.buckets.len(), 0);
}

#[test]
fn test_stats() {
    let mut filter = TTLBloomFilter::new(3600, 900, 0.01, 1000);

    // Add items
    for i in 0..5 {
        filter.add(&format!("item-{}", i));
    }

    let stats = filter.stats();

    assert_eq!(stats.total_items, 5);
    assert_eq!(stats.total_buckets, 1);
    assert_eq!(stats.default_ttl_secs, 3600);
    assert_eq!(stats.bucket_duration_secs, 900);
    assert_eq!(stats.false_positive_rate, 0.01);
    assert_eq!(stats.buckets.len(), 1);
}

#[test]
fn test_bucket_expiration_timing() {
    let mut filter = TTLBloomFilter::new(3600, 900, 0.01, 1000);

    filter.add("test-item");

    let bucket = filter.buckets.values().next().unwrap();
    let expires_at = bucket.expires_at;
    let current_time = TTLBloomFilter::current_timestamp();

    // Bucket should expire after TTL
    let time_until_expiration = expires_at - current_time;

    // Should be approximately TTL (within bucket duration)
    assert!(time_until_expiration >= 3600);
    assert!(time_until_expiration < 3600 + 900);
}

#[test]
fn test_different_ttls() {
    // Test various TTL configurations
    let configs = vec![
        (900, 180),    // 15 min TTL, 3 min buckets
        (3600, 900),   // 1 hour TTL, 15 min buckets
        (86400, 3600), // 24 hour TTL, 1 hour buckets
    ];

    for (ttl, bucket_duration) in configs {
        let filter = TTLBloomFilter::new(ttl, bucket_duration, 0.01, 1000);
        assert_eq!(filter.metadata.default_ttl_secs, ttl);
        assert_eq!(filter.metadata.bucket_duration_secs, bucket_duration);
    }
}

// ============================================================================
// SERIALIZATION TESTS
// ============================================================================

#[test]
fn test_serialization_deserialization() {
    let mut filter = TTLBloomFilter::new(3600, 900, 0.01, 1000);

    // Add some items
    for i in 0..10 {
        filter.add(&format!("item-{}", i));
    }

    // Serialize
    let serialized = filter.to_base64().expect("Serialization failed");
    assert!(!serialized.is_empty());

    // Deserialize
    let deserialized = TTLBloomFilter::from_base64(&serialized).expect("Deserialization failed");

    // Check metadata
    assert_eq!(deserialized.metadata.default_ttl_secs, 3600);
    assert_eq!(deserialized.metadata.bucket_duration_secs, 900);
    assert_eq!(deserialized.buckets.len(), 1);

    // Check items are still there
    for i in 0..10 {
        assert!(deserialized
            .buckets
            .values()
            .next()
            .unwrap()
            .filter
            .contains(&format!("item-{}", i)));
    }
}

#[test]
fn test_empty_filter_serialization() {
    let filter = TTLBloomFilter::new(3600, 900, 0.01, 1000);

    // Serialize empty filter
    let serialized = filter.to_base64().expect("Serialization failed");

    // Deserialize
    let deserialized = TTLBloomFilter::from_base64(&serialized).expect("Deserialization failed");

    assert_eq!(deserialized.buckets.len(), 0);
    assert_eq!(deserialized.metadata.default_ttl_secs, 3600);
}

#[test]
fn test_invalid_base64() {
    let result = TTLBloomFilter::from_base64("invalid-base64!!!");
    assert!(result.is_err());
}

#[test]
fn test_roundtrip_serialization() {
    let mut filter = TTLBloomFilter::new(7200, 1800, 0.005, 5000);

    // Add many items
    for i in 0..100 {
        filter.add(&format!("jwt-{}", i));
    }

    // Serialize
    let serialized = filter.to_base64().unwrap();

    // Deserialize
    let mut deserialized = TTLBloomFilter::from_base64(&serialized).unwrap();

    // Verify all items are still findable
    for i in 0..100 {
        assert!(deserialized.check(&format!("jwt-{}", i)));
    }

    // Verify metadata matches
    assert_eq!(
        filter.metadata.default_ttl_secs,
        deserialized.metadata.default_ttl_secs
    );
    assert_eq!(
        filter.metadata.bucket_duration_secs,
        deserialized.metadata.bucket_duration_secs
    );
}
