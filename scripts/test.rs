use solana_program::pubkey::Pubkey;
use std::str::FromStr;

fn main() {
    let owner_str = "C7GKpfqQyBoFR6S13DECwBjdi7aCQKbbeKjXm4Jt5Hds";
    let owner_pubkey = Pubkey::from_str(owner_str)
        .expect("Invalid public key");
    let owner_bytes = owner_pubkey.to_bytes();
    println!("{:?}", owner_bytes); // This prints the 32-byte array.
}
