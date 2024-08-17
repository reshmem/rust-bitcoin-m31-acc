pub fn m31_to_limbs(v: u32) -> [i64; 4] {
    [
        (v & 255) as i64,
        ((v >> 8) & 255) as i64,
        ((v >> 16) & 255) as i64,
        ((v >> 24) & 255) as i64,
    ]
}
