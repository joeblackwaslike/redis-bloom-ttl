.PHONY: test-unit test-integration test-all

# Unit tests - No Redis required
test-unit:
	@echo "🧪 Running unit tests (no Redis required)..."
	cargo test --test unit_test -- --nocapture

# Integration tests - Redis required
test-integration:
	@echo "🧪 Running integration tests (Redis required)..."
	@echo "Make sure Redis is running with: make redis-start"
	cargo test --test integration_test -- --nocapture

# All tests
test-all:
	@echo "🧪 Running all tests..."
	@$(MAKE) test-unit
	@echo ""
	@$(MAKE) test-integration

# Quick unit tests during development
watch-unit:
	cargo watch -x 'test --test unit_tests'