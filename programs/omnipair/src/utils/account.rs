/// Calculates the total size needed for an account including the 8-byte discriminator
/// @notice This function adds the 8-byte discriminator to the size of a generic type T
/// @dev Uses std::mem::size_of to get the size of type T at compile time
/// @return usize The total size in bytes needed for the account
pub fn get_size_with_discriminator<T>() -> usize {
    8 + std::mem::size_of::<T>()
}

/// Calculates the total size needed for an account with a custom size plus the 8-byte discriminator
/// @notice This function adds the 8-byte discriminator to a provided custom size
/// @param custom_size The custom size in bytes needed for the account data
/// @return usize The total size in bytes needed for the account
pub fn get_size_with_discriminator_and_custom_size(custom_size: usize) -> usize {
    8 + custom_size
} 