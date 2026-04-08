// ── Remaining-accounts layout ─────────────────────────────────────────────────
//
// Shared by multiply and close_multiply. Passed verbatim from the outer
// instruction → omnipair flashloan → flash_loan_callback.
//
// For multiply (open):  TOKEN_IN = lev_collateral reserve, TOKEN_OUT = position token reserve.
// For close_multiply:   TOKEN_IN = position token reserve, TOKEN_OUT = lev_collateral reserve.
//
// Index  Account                   Writable
// 0      pair                       yes
// 1      rate_model                 yes
// 2      futarchy_authority         no
// 3      user_position              yes
// 4      token_in_reserve_vault     yes
// 5      token_out_reserve_vault    yes
// 6      collateral_vault           yes   (position-token collateral vault)
// 7      token_2022_program         no
// 8      system_program             no
// 9      event_authority            no    (omnipair's __event_authority PDA)
// 10     omnipair_program           no
// 11     user_leverage_position     yes
pub const IDX_PAIR: usize = 0;
pub const IDX_RATE_MODEL: usize = 1;
pub const IDX_FUTARCHY: usize = 2;
pub const IDX_USER_POSITION: usize = 3;
pub const IDX_TOKEN_IN_VAULT: usize = 4;
pub const IDX_TOKEN_OUT_VAULT: usize = 5;
pub const IDX_COLLATERAL_VAULT: usize = 6;
pub const IDX_TOKEN_2022_PROGRAM: usize = 7;
pub const IDX_SYSTEM_PROGRAM: usize = 8;
pub const IDX_EVENT_AUTHORITY: usize = 9;
pub const IDX_OMNIPAIR_PROGRAM: usize = 10;
pub const IDX_USER_LEV_POSITION: usize = 11;

pub const BPS_DENOMINATOR: u64 = 10_000;
pub const FLASHLOAN_FEE_BPS: u64 = 5;
