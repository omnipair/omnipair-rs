import type { IdlAccounts, IdlEvents } from "@coral-xyz/anchor";
import type { Omnipair } from "./types.js";
import type { OmnipairV2 } from "./types_v2.js";

export type Pair = IdlAccounts<Omnipair>["pair"];
export type UserPosition = IdlAccounts<Omnipair>["userPosition"];
export type RateModel = IdlAccounts<Omnipair>["rateModel"];
export type FutarchyAuthority = IdlAccounts<Omnipair>["futarchyAuthority"];

export type Market = IdlAccounts<OmnipairV2>["market"];
export type MarginPosition = IdlAccounts<OmnipairV2>["marginPosition"];
export type YieldAccount = IdlAccounts<OmnipairV2>["yieldAccount"];
export type V2FutarchyAuthority = IdlAccounts<OmnipairV2>["futarchyAuthority"];

export type HlpClosed = IdlEvents<OmnipairV2>["hlpClosed"];
export type HlpOpened = IdlEvents<OmnipairV2>["hlpOpened"];
export type HlpRebalanced = IdlEvents<OmnipairV2>["hlpRebalanced"];
export type LiquidityAdded = IdlEvents<OmnipairV2>["liquidityAdded"];
export type LiquidityRemoved = IdlEvents<OmnipairV2>["liquidityRemoved"];
export type MarketCollateralDeposited = IdlEvents<OmnipairV2>["marketCollateralDeposited"];
export type MarketCollateralWithdrawn = IdlEvents<OmnipairV2>["marketCollateralWithdrawn"];
export type MarketCreated = IdlEvents<OmnipairV2>["marketCreated"];
export type MarketDebtUpdated = IdlEvents<OmnipairV2>["marketDebtUpdated"];
export type MarketFeeLiabilityClaimed = IdlEvents<OmnipairV2>["marketFeeLiabilityClaimed"];
export type MarketHealthUpdated = IdlEvents<OmnipairV2>["marketHealthUpdated"];
export type MarketUpdated = IdlEvents<OmnipairV2>["marketUpdated"];
export type PositionLiquidated = IdlEvents<OmnipairV2>["positionLiquidated"];
export type ProtocolAuctionSettled = IdlEvents<OmnipairV2>["protocolAuctionSettled"];
export type SwapExecuted = IdlEvents<OmnipairV2>["swapExecuted"];
export type YieldClaimed = IdlEvents<OmnipairV2>["yieldClaimed"];
export type YieldRecipientUpdated = IdlEvents<OmnipairV2>["yieldRecipientUpdated"];
