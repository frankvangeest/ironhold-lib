set RUST_BACKTRACE=1
set RUST_LOG=bevy_render=info,bevy_ecs=trace
set WGPU_DEBUG=1
set VERBOSE_SHADER_ERROR=1
set BEVY_BACKTRACE=full
cargo test -p ironhold_core --test ron_validation