// TODO: Implement fee collection without burning LP principal.
//
// - Track global fee growth for each token (e.g., fee_growth_global_token0, fee_growth_global_token1).
// - For each LP, store their last recorded fee growth snapshot (e.g., user_fee_growth_token0, user_fee_growth_token1).
// - When swaps occur, increment fee_growth_global_* variables proportional to fees collected.
// - On mint/burn/collect, update user's claimable fees based on (current_global - last_snapshot) * user_liquidity.
// - Add a `collect_fees` instruction allowing users to harvest fees without touching their liquidity position.
// - Ensure `burn` instruction also collects any pending fees before burning LP tokens.
//
// Note: Keep the pool math xy=k unchanged (no concentrated liquidity logic).
