pub fn m31_to_limbs(v: u32) -> [i32; 4] {
    [
        (v & 255) as i32,
        ((v >> 8) & 255) as i32,
        ((v >> 16) & 255) as i32,
        ((v >> 24) & 255) as i32,
    ]
}
