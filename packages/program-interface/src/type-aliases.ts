import type { IdlAccounts, IdlEvents } from "@coral-xyz/anchor";
import type { Omnipair } from "./types.js";
import type { OmnipairV2 } from "./types_v2.js";

export type Pair = IdlAccounts<Omnipair>["pair"];
export type UserPosition = IdlAccounts<Omnipair>["userPosition"];
export type RateModel = IdlAccounts<Omnipair>["rateModel"];
export type FutarchyAuthority = IdlAccounts<Omnipair>["futarchyAuthority"];

export type Market = IdlAccounts<OmnipairV2>["market"];
export type StakePosition = IdlAccounts<OmnipairV2>["stakePosition"];
export type MarginPosition = IdlAccounts<OmnipairV2>["marginPosition"];
export type HedgePosition = IdlAccounts<OmnipairV2>["hedgePosition"];

export type LiquidityAdded = IdlEvents<OmnipairV2>["liquidityAdded"];
export type LiquidityRemoved = IdlEvents<OmnipairV2>["liquidityRemoved"];
export type MarketCollateralDeposited = IdlEvents<OmnipairV2>["marketCollateralDeposited"];
export type MarketCollateralWithdrawn = IdlEvents<OmnipairV2>["marketCollateralWithdrawn"];
export type MarketCreated = IdlEvents<OmnipairV2>["marketCreated"];
export type MarketDebtUpdated = IdlEvents<OmnipairV2>["marketDebtUpdated"];
export type MarketFeeLiabilityClaimed = IdlEvents<OmnipairV2>["marketFeeLiabilityClaimed"];
export type MarketFeesClaimed = IdlEvents<OmnipairV2>["marketFeesClaimed"];
export type MarketHealthUpdated = IdlEvents<OmnipairV2>["marketHealthUpdated"];
export type MarketHedgeClosed = IdlEvents<OmnipairV2>["marketHedgeClosed"];
export type MarketHedgeFeesClaimed = IdlEvents<OmnipairV2>["marketHedgeFeesClaimed"];
export type MarketHedgeOpened = IdlEvents<OmnipairV2>["marketHedgeOpened"];
export type MarketInsuranceFunded = IdlEvents<OmnipairV2>["marketInsuranceFunded"];
export type MarketStakeUpdated = IdlEvents<OmnipairV2>["marketStakeUpdated"];
export type MarketUpdated = IdlEvents<OmnipairV2>["marketUpdated"];
export type PositionLiquidated = IdlEvents<OmnipairV2>["positionLiquidated"];
export type SwapExecuted = IdlEvents<OmnipairV2>["swapExecuted"];
