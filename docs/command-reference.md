## Updated Command Reference

### `TTLBLOOM.CREATE`
**Syntax:** `TTLBLOOM.CREATE key default_ttl_secs bucket_duration_secs false_positive_rate expected_items_per_bucket`

**Parameters:**
- `key` - Redis key name for the filter
- `default_ttl_secs` - Default TTL in seconds for all items (e.g., 3600 for 1 hour)
- `bucket_duration_secs` - Size of each time bucket (e.g., 900 for 15 minutes)
- `false_positive_rate` - Target false positive rate (e.g., 0.01 for 1%)
- `expected_items_per_bucket` - Expected number of items per bucket

**Returns:** `OK` on success

### `TTLBLOOM.ADD`
**Syntax:** `TTLBLOOM.ADD key item`

**Parameters:**
- `key` - Redis key name of the filter
- `item` - Item to add (e.g., JWT JTI)

**Returns:** `1` on success

### `TTLBLOOM.CHECK`
**Syntax:** `TTLBLOOM.CHECK key item`

**Parameters:**
- `key` - Redis key name of the filter
- `item` - Item to check

**Returns:** `1` if item exists (possibly false positive), `0` if definitely doesn't exist

### `TTLBLOOM.STATS`
**Syntax:** `TTLBLOOM.STATS key`

**Parameters:**
- `key` - Redis key name of the filter

**Returns:** JSON string with statistics including `default_ttl_secs`

### `TTLBLOOM.CLEANUP`
**Syntax:** `TTLBLOOM.CLEANUP key`

**Parameters:**
- `key` - Redis key name of the filter

**Returns:** Number of expired buckets removed
