//! Pieces of the E5 relay harness shared by the native driver and the browser page: the probe payload format and the percentile
//! helper. Pure code, no clock and no I/O.

pub const OP_LOAD: i64 = 1;
pub const OP_PROBE: i64 = 2;
pub const OP_ECHO: i64 = 3;

/// 16-byte header (sequence, scheduled send time in microseconds since the prober's run start) padded to `size` bytes.
pub fn payload(seq: u64, sched_us: u64, size: usize) -> Vec<u8> {
    let mut v = vec![0u8; size.max(16)];
    v[..8].copy_from_slice(&seq.to_le_bytes());
    v[8..16].copy_from_slice(&sched_us.to_le_bytes());
    v
}

pub fn sched_of(bytes: &[u8]) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(8..16)?.try_into().ok()?))
}

/// Nearest-rank percentile of an ascending slice.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = ((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_round_trip() {
        let p = payload(7, 123_456, 64);
        assert_eq!(p.len(), 64);
        assert_eq!(sched_of(&p), Some(123_456));
    }

    #[test]
    fn percentile_nearest_rank() {
        let v: Vec<f64> = (1..=100).map(|n| n as f64).collect();
        assert_eq!(percentile(&v, 0.5), 50.0);
        assert_eq!(percentile(&v, 0.95), 95.0);
        assert_eq!(percentile(&v, 0.99), 99.0);
    }
}
