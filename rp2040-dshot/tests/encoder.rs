
use rp2040_dshot::encoder::DShotSpeed;

#[test]
fn test_gcr_rate_ratio() {
    // GCR encoding ratio should be exactly 1.25 (5/4)
    for speed in [
        DShotSpeed::DShot150,
        DShotSpeed::DShot300,
        DShotSpeed::DShot600,
        DShotSpeed::DShot1200,
    ] {
        let normal_rate = speed.bit_rate_hz();
        let gcr_rate = speed.gcr_bit_rate_hz();
        let ratio = gcr_rate as f32 / normal_rate as f32;
        
        assert!(
            (ratio - 1.25).abs() < 0.01,
            "GCR ratio should be 1.25 for {:?}, got {}",
            speed,
            ratio
        );
    }
}