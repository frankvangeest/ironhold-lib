set RUST_BACKTRACE=1
@REM set RUST_LOG=debug
set RUST_LOG=bevy_render=info,bevy_ecs=trace
@REM set RUST_LOG=bevy_ecs=debug,bevy_app=debug,bevy=info
set WGPU_DEBUG=1
set VERBOSE_SHADER_ERROR=1
set BEVY_BACKTRACE=full
cargo test -p ironhold_core
@REM cargo test -p ironhold_core --test integration_tests
