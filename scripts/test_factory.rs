use solana_program::pubkey::Pubkey;
use std::str::FromStr;

fn main() {
    // Test the factory initialization logic
    
    // Example owner public key
    let owner_str = "C7GKpfqQyBoFR6S13DECwBjdi7aCQKbbeKjXm4Jt5Hds";
    let owner_pubkey = Pubkey::from_str(owner_str)
        .expect("Invalid public key");
    
    // Print the owner's public key bytes
    let owner_bytes = owner_pubkey.to_bytes();
    println!("Owner public key bytes: {:?}", owner_bytes);
    
    // Calculate the factory PDA
    let program_id = Pubkey::from_str("11111111111111111111111111111111")
        .expect("Invalid program ID");
    
    let (factory_pda, bump) = Pubkey::find_program_address(
        &[b"factory", owner_bytes.as_ref()],
        &program_id
    );
    
    println!("Factory PDA: {}", factory_pda);
    println!("Factory PDA bump: {}", bump);
    
    // Calculate the size of the factory account
    const MAX_PAIRS: usize = 1000;
    const SIZE: usize = 8 + 32 + 8 + 4 + (MAX_PAIRS * 32);
    
    println!("Factory account size: {} bytes", SIZE);
    println!("Maximum number of pairs: {}", MAX_PAIRS);
} 