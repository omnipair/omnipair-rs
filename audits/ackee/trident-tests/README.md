## Trident fuzz tests (quick start)

### Build programs (required `.so` artifacts)
- Build from the program repo root (one level up):
  - Anchor: `anchor build`
  - Or Solana: `cargo build-sbf`
- Ensure `../target/deploy/omnipair.so` exists (path is in `Trident.toml`).
- Instruction-level flashloan support was dropped from the current program scope; see `FLASHLOAN_DROPPED.md`.

### Install Trident CLI
```bash
cargo install trident-cli
```

### Run fuzz tests (from this folder)
```bash
trident fuzz run fuzz
trident fuzz run fuzz_lending
trident fuzz run fuzz_liquidity_swaps
```

### Tweak behavior
- Iterations and flows/iteration: edit `FuzzTest::fuzz(iterations, flows_per_iteration)` at the bottom of each `test_fuzz.rs`.
- Flow selection: adjust `#[flow(weight = N)]` on flow methods. Higher weight = more frequent.

### Docs
- Trident Documentation: [https://ackee.xyz/trident/docs/latest/](https://ackee.xyz/trident/docs/latest/)
