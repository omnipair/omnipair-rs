use anchor_lang::Space;

/// Calculates the total size needed for an account including the 8-byte discriminator.
/// 
/// @notice This function adds the 8-byte discriminator to the INIT_SPACE of type T.
/// @dev Requires T to implement the `Space` trait (via `#[derive(InitSpace)]`).
///      This correctly calculates Borsh-serialized sizes for all types including
///      `Vec`, `String`, `Option`, and `Enum` fields.
/// @return usize The total size in bytes needed for the account
pub fn get_size_with_discriminator<T: Space>() -> usize {
    8 + T::INIT_SPACE
}

/// Calculates the total size needed for an account with a custom size plus the 8-byte discriminator
/// @notice This function adds the 8-byte discriminator to a provided custom size
/// @param custom_size The custom size in bytes needed for the account data
/// @return usize The total size in bytes needed for the account
pub fn get_size_with_discriminator_and_custom_size(custom_size: usize) -> usize {
    8 + custom_size
} 