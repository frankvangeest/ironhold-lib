# ironhold_core — Integration Test Rules

Tests in `ironhold_core/tests/` must:
- Include `PhysicsPlugin` (missing it causes panics from unregistered physics resources).
- Initialize the `Message` framework (Writer/Reader resources) before running any messaging systems.

See `tests/support.rs` for the `setup_test_app()` helper.
