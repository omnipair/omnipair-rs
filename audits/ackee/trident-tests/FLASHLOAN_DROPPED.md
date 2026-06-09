# Flashloan Instruction Support Dropped

The current Omnipair program no longer exposes the instruction-level flashloan API or the standalone flashloan receiver example. The Trident fuzz suites remain in place for liquidity, swap, lending, and liquidation coverage, but active flashloan flows are disabled for this branch.

Historical generated Trident types may still mention flashloan until the harness is regenerated from a fresh IDL.
