

/// Compares Golden Traces against FastGPU behavior
pub fn validate_abi_struct_layout(struct_id: u32, expected_size: usize, actual_size: usize) -> bool {
    expected_size == actual_size
}
