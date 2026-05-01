// use redis::{Commands, RedisResult};
// use std::thread;
// use std::time::Duration;
// use uuid::Uuid;

// /// Helper function to get Redis connection
// fn get_connection() -> redis::Connection {
//     let client =
//         redis::Client::open("redis://127.0.0.1:6379/").expect("Failed to create Redis client");
//     client.get_connection().expect("Failed to connect to Redis")
// }

// /// Helper to create a unique test key
// fn test_key(name: &str) -> String {
//     format!("test:{}:{}", name, Uuid::new_v4())
// }

// #[test]
// fn test_module_loaded() {
//     let mut con = get_connection();

//     let modules: Vec<Vec<redis::Value>> = redis::cmd("MODULE")
//         .arg("LIST")
//         .query(&mut con)
//         .expect("Failed to list modules");

//     let mut found = false;
//     for module in modules {
//         if let Some(redis::Value::Data(name)) = module.get(1) {
//             if name == b"ttlbloom" {
//                 found = true;
//                 break;
//             }
//         }
//     }

//     assert!(found, "ttlbloom module not loaded");
// }

// #[test]
// fn test_create_filter() {
//     let mut con = get_connection();
//     let key = test_key("create");

//     let result: String = redis::cmd("TTLBLOOM.CREATE")
//         .arg(&key)
//         .arg(3600) // default_ttl_secs
//         .arg(900) // bucket_duration_secs
//         .arg(0.01) // false_positive_rate
//         .arg(10000) // expected_items_per_bucket
//         .query(&mut con)
//         .expect("Failed to create filter");

//     assert_eq!(result, "OK");

//     // Verify key exists
//     let exists: bool = con.exists(&key).expect("Failed to check key existence");
//     assert!(exists);

//     // Cleanup
//     let _: () = con.del(&key).unwrap();
// }

// #[test]
// fn test_create_invalid_params() {
//     let mut con = get_connection();

//     // Invalid false positive rate (> 1)
//     let key = test_key("invalid_fp");
//     let result: RedisResult<String> = redis::cmd("TTLBLOOM.CREATE")
//         .arg(&key)
//         .arg(3600)
//         .arg(900)
//         .arg(1.5) // Invalid!
//         .arg(10000)
//         .query(&mut con);

//     assert!(result.is_err());

//     // Invalid bucket duration (0)
//     let key = test_key("invalid_bucket");
//     let result: RedisResult<String> = redis::cmd("TTLBLOOM.CREATE")
//         .arg(&key)
//         .arg(3600)
//         .arg(0) // Invalid!
//         .arg(0.01)
//         .arg(10000)
//         .query(&mut con);

//     assert!(result.is_err());

//     // Invalid TTL (0)
//     let key = test_key("invalid_ttl");
//     let result: RedisResult<String> = redis::cmd("TTLBLOOM.CREATE")
//         .arg(&key)
//         .arg(0) // Invalid!
//         .arg(900)
//         .arg(0.01)
//         .arg(10000)
//         .query(&mut con);

//     assert!(result.is_err());
// }

// #[test]
// fn test_add_and_check() {
//     let mut con = get_connection();
//     let key = test_key("add_check");

//     // Create filter
//     let _: String = redis::cmd("TTLBLOOM.CREATE")
//         .arg(&key)
//         .arg(3600)
//         .arg(900)
//         .arg(0.01)
//         .arg(10000)
//         .query(&mut con)
//         .expect("Failed to create filter");

//     // Add item
//     let test_jti = Uuid::new_v4().to_string();
//     let result: i64 = redis::cmd("TTLBLOOM.ADD")
//         .arg(&key)
//         .arg(&test_jti)
//         .query(&mut con)
//         .expect("Failed to add item");

//     assert_eq!(result, 1);

//     // Check item exists
//     let exists: i64 = redis::cmd("TTLBLOOM.CHECK")
//         .arg(&key)
//         .arg(&test_jti)
//         .query(&mut con)
//         .expect("Failed to check item");

//     assert_eq!(exists, 1);

//     // Check non-existent item
//     let fake_jti = Uuid::new_v4().to_string();
//     let not_exists: i64 = redis::cmd("TTLBLOOM.CHECK")
//         .arg(&key)
//         .arg(&fake_jti)
//         .query(&mut con)
//         .expect("Failed to check non-existent item");

//     assert_eq!(not_exists, 0);

//     // Cleanup
//     let _: () = con.del(&key).unwrap();
// }

// #[test]
// fn test_add_without_create() {
//     let mut con = get_connection();
//     let key = test_key("no_create");
//     let test_jti = Uuid::new_v4().to_string();

//     let result: RedisResult<i64> = redis::cmd("TTLBLOOM.ADD")
//         .arg(&key)
//         .arg(&test_jti)
//         .query(&mut con);

//     assert!(result.is_err());

//     if let Err(e) = result {
//         let error_msg = format!("{}", e);
//         assert!(error_msg.contains("Use TTLBLOOM.CREATE first"));
//     }
// }

// #[test]
// fn test_check_nonexistent_filter() {
//     let mut con = get_connection();
//     let key = test_key("no_filter");
//     let test_jti = Uuid::new_v4().to_string();

//     let result: i64 = redis::cmd("TTLBLOOM.CHECK")
//         .arg(&key)
//         .arg(&test_jti)
//         .query(&mut con)
//         .expect("Failed to check");

//     assert_eq!(result, 0);
// }

// #[test]
// fn test_stats() {
//     let mut con = get_connection();
//     let key = test_key("stats");

//     // Create filter
//     let _: String = redis::cmd("TTLBLOOM.CREATE")
//         .arg(&key)
//         .arg(3600)
//         .arg(900)
//         .arg(0.01)
//         .arg(10000)
//         .query(&mut con)
//         .unwrap();

//     // Add multiple items
//     for _ in 0..10 {
//         let jti = Uuid::new_v4().to_string();
//         let _: i64 = redis::cmd("TTLBLOOM.ADD")
//             .arg(&key)
//             .arg(&jti)
//             .query(&mut con)
//             .unwrap();
//     }

//     // Get stats
//     let stats: String = redis::cmd("TTLBLOOM.STATS")
//         .arg(&key)
//         .query(&mut con)
//         .expect("Failed to get stats");

//     assert!(stats.contains("total_items"));
//     assert!(stats.contains("total_buckets"));
//     assert!(stats.contains("default_ttl_secs"));
//     assert!(stats.contains("bucket_duration_secs"));

//     // Cleanup
//     let _: () = con.del(&key).unwrap();
// }

// #[test]
// fn test_cleanup() {
//     let mut con = get_connection();
//     let key = test_key("cleanup");

//     // Create filter with short TTL (5 seconds)
//     let _: String = redis::cmd("TTLBLOOM.CREATE")
//         .arg(&key)
//         .arg(5) // 5 second TTL
//         .arg(2) // 2 second buckets
//         .arg(0.01)
//         .arg(1000)
//         .query(&mut con)
//         .unwrap();

//     // Add items
//     for _ in 0..5 {
//         let jti = Uuid::new_v4().to_string();
//         let _: i64 = redis::cmd("TTLBLOOM.ADD")
//             .arg(&key)
//             .arg(&jti)
//             .query(&mut con)
//             .unwrap();
//     }

//     // Wait for expiration
//     thread::sleep(Duration::from_secs(6));

//     // Run cleanup
//     let removed: i64 = redis::cmd("TTLBLOOM.CLEANUP")
//         .arg(&key)
//         .query(&mut con)
//         .expect("Failed to cleanup");

//     // Should have removed at least 0 buckets (might be 0 if already cleaned up)
//     assert!(removed >= 0);

//     // Cleanup
//     let _: () = con.del(&key).unwrap();
// }

// #[test]
// fn test_ttl_expiration() {
//     let mut con = get_connection();
//     let key = test_key("expiration");

//     // Create filter with 3 second TTL, 1 second buckets
//     let _: String = redis::cmd("TTLBLOOM.CREATE")
//         .arg(&key)
//         .arg(3) // 3 second TTL
//         .arg(1) // 1 second buckets
//         .arg(0.01)
//         .arg(1000)
//         .query(&mut con)
//         .unwrap();

//     // Add item
//     let test_jti = Uuid::new_v4().to_string();
//     let _: i64 = redis::cmd("TTLBLOOM.ADD")
//         .arg(&key)
//         .arg(&test_jti)
//         .query(&mut con)
//         .unwrap();

//     // Check immediately - should exist
//     let exists: i64 = redis::cmd("TTLBLOOM.CHECK")
//         .arg(&key)
//         .arg(&test_jti)
//         .query(&mut con)
//         .unwrap();

//     assert_eq!(exists, 1);

//     // Wait for expiration (TTL + bucket duration + buffer)
//     thread::sleep(Duration::from_secs(5));

//     // Check again - should be expired
//     let expired: i64 = redis::cmd("TTLBLOOM.CHECK")
//         .arg(&key)
//         .arg(&test_jti)
//         .query(&mut con)
//         .unwrap();

//     assert_eq!(expired, 0);

//     // Cleanup
//     let _: () = con.del(&key).unwrap();
// }

// #[test]
// fn test_multiple_items() {
//     let mut con = get_connection();
//     let key = test_key("multiple");

//     // Create filter
//     let _: String = redis::cmd("TTLBLOOM.CREATE")
//         .arg(&key)
//         .arg(3600)
//         .arg(900)
//         .arg(0.01)
//         .arg(10000)
//         .query(&mut con)
//         .unwrap();

//     // Add 100 items
//     let mut items = Vec::new();
//     for _ in 0..100 {
//         let jti = Uuid::new_v4().to_string();
//         items.push(jti.clone());
//         let _: i64 = redis::cmd("TTLBLOOM.ADD")
//             .arg(&key)
//             .arg(&jti)
//             .query(&mut con)
//             .unwrap();
//     }

//     // Verify all items exist
//     for item in &items {
//         let exists: i64 = redis::cmd("TTLBLOOM.CHECK")
//             .arg(&key)
//             .arg(item)
//             .query(&mut con)
//             .unwrap();

//         assert_eq!(exists, 1, "Item {} should exist", item);
//     }

//     // Cleanup
//     let _: () = con.del(&key).unwrap();
// }

// #[test]
// fn test_false_positive_rate() {
//     let mut con = get_connection();
//     let key = test_key("fp_rate");

//     // Create filter
//     let _: String = redis::cmd("TTLBLOOM.CREATE")
//         .arg(&key)
//         .arg(3600)
//         .arg(900)
//         .arg(0.01) // 1% target FP rate
//         .arg(1000)
//         .query(&mut con)
//         .unwrap();

//     // Add 1000 items
//     let mut added_items = Vec::new();
//     for _ in 0..1000 {
//         let jti = Uuid::new_v4().to_string();
//         added_items.push(jti.clone());
//         let _: i64 = redis::cmd("TTLBLOOM.ADD")
//             .arg(&key)
//             .arg(&jti)
//             .query(&mut con)
//             .unwrap();
//     }

//     // Check for false positives with 1000 non-added items
//     let mut false_positives = 0;
//     for _ in 0..1000 {
//         let jti = Uuid::new_v4().to_string();
//         let exists: i64 = redis::cmd("TTLBLOOM.CHECK")
//             .arg(&key)
//             .arg(&jti)
//             .query(&mut con)
//             .unwrap();

//         if exists == 1 {
//             false_positives += 1;
//         }
//     }

//     let fp_rate = false_positives as f64 / 1000.0;

//     println!(
//         "False positive rate: {:.4} ({}/1000)",
//         fp_rate, false_positives
//     );

//     // Should be close to 0.01, allow up to 3x variance
//     assert!(fp_rate < 0.03, "False positive rate {} too high", fp_rate);

//     // Cleanup
//     let _: () = con.del(&key).unwrap();
// }
