/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/omnipair_v2.json`.
 */
export type OmnipairV2 = {
  "address": "358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv",
  "metadata": {
    "name": "omnipairV2",
    "version": "0.10.2",
    "spec": "0.1.0",
    "description": "Omnipair v2 market architecture program"
  },
  "instructions": [
    {
      "name": "addLiquidity",
      "discriminator": [
        181,
        157,
        89,
        67,
        143,
        182,
        52,
        72
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "claimTokenMint",
          "writable": true
        },
        {
          "name": "reserveVault",
          "writable": true
        },
        {
          "name": "ownerAssetAccount",
          "writable": true
        },
        {
          "name": "ownerClaimAccount",
          "writable": true
        },
        {
          "name": "stakePosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  116,
                  97,
                  107,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "owner"
              },
              {
                "kind": "account",
                "path": "assetMint"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "addLiquidityArgs"
            }
          }
        }
      ]
    },
    {
      "name": "borrow",
      "discriminator": [
        228,
        253,
        131,
        202,
        207,
        116,
        89,
        18
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "debtAssetMint"
        },
        {
          "name": "collateralAssetMint"
        },
        {
          "name": "reserveVault",
          "writable": true
        },
        {
          "name": "ownerDebtAccount",
          "writable": true
        },
        {
          "name": "marginPosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  103,
                  105,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "owner"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "borrowArgs"
            }
          }
        }
      ]
    },
    {
      "name": "claimFees",
      "discriminator": [
        82,
        251,
        233,
        156,
        12,
        52,
        184,
        202
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "feeVault",
          "writable": true
        },
        {
          "name": "ownerFeeAccount",
          "writable": true
        },
        {
          "name": "stakePosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  116,
                  97,
                  107,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "owner"
              },
              {
                "kind": "account",
                "path": "assetMint"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "claimFeesArgs"
            }
          }
        }
      ]
    },
    {
      "name": "claimHedgeFees",
      "discriminator": [
        169,
        148,
        87,
        149,
        188,
        246,
        204,
        210
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "feeVault",
          "writable": true
        },
        {
          "name": "ownerFeeAccount",
          "writable": true
        },
        {
          "name": "hedgePosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  101,
                  100,
                  103,
                  101,
                  95,
                  112,
                  111,
                  115,
                  105,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "owner"
              },
              {
                "kind": "account",
                "path": "assetMint"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "claimHedgeFeesArgs"
            }
          }
        }
      ]
    },
    {
      "name": "claimMarketFees",
      "discriminator": [
        181,
        120,
        254,
        224,
        232,
        113,
        48,
        221
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "feeAuthority",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "feeVault",
          "writable": true
        },
        {
          "name": "recipientFeeAccount",
          "writable": true
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "claimMarketFeesArgs"
            }
          }
        }
      ]
    },
    {
      "name": "closeHedge",
      "discriminator": [
        223,
        109,
        6,
        229,
        136,
        160,
        43,
        47
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "claimTokenMint"
        },
        {
          "name": "hedgeTokenMint",
          "writable": true
        },
        {
          "name": "hedgeVault",
          "writable": true
        },
        {
          "name": "ownerClaimAccount",
          "writable": true
        },
        {
          "name": "ownerHedgeAccount",
          "writable": true
        },
        {
          "name": "hedgePosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  101,
                  100,
                  103,
                  101,
                  95,
                  112,
                  111,
                  115,
                  105,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "owner"
              },
              {
                "kind": "account",
                "path": "assetMint"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "closeHedgeArgs"
            }
          }
        }
      ]
    },
    {
      "name": "depositCollateral",
      "discriminator": [
        156,
        131,
        142,
        116,
        146,
        247,
        162,
        120
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "collateralVault",
          "writable": true
        },
        {
          "name": "ownerAssetAccount",
          "writable": true
        },
        {
          "name": "marginPosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  103,
                  105,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "owner"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "depositCollateralArgs"
            }
          }
        }
      ]
    },
    {
      "name": "depositInsurance",
      "discriminator": [
        34,
        221,
        238,
        103,
        190,
        136,
        23,
        194
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "sponsor",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "insuranceVault",
          "writable": true
        },
        {
          "name": "sponsorAssetAccount",
          "writable": true
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "depositInsuranceArgs"
            }
          }
        }
      ]
    },
    {
      "name": "initialize",
      "discriminator": [
        175,
        175,
        109,
        31,
        13,
        152,
        155,
        237
      ],
      "accounts": [
        {
          "name": "payer",
          "writable": true,
          "signer": true
        },
        {
          "name": "baseMint"
        },
        {
          "name": "quoteMint"
        },
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "baseMint"
              },
              {
                "kind": "account",
                "path": "quoteMint"
              },
              {
                "kind": "arg",
                "path": "args.params_hash"
              }
            ]
          }
        },
        {
          "name": "baseClaimTokenMint"
        },
        {
          "name": "quoteClaimTokenMint"
        },
        {
          "name": "baseHedgeTokenMint"
        },
        {
          "name": "quoteHedgeTokenMint"
        },
        {
          "name": "baseHedgeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  101,
                  100,
                  103,
                  101,
                  100
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "baseClaimTokenMint"
              }
            ]
          }
        },
        {
          "name": "quoteHedgeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  101,
                  100,
                  103,
                  101,
                  100
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "quoteClaimTokenMint"
              }
            ]
          }
        },
        {
          "name": "baseReserveVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  114,
                  101,
                  115,
                  101,
                  114,
                  118,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "baseMint"
              }
            ]
          }
        },
        {
          "name": "quoteReserveVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  114,
                  101,
                  115,
                  101,
                  114,
                  118,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "quoteMint"
              }
            ]
          }
        },
        {
          "name": "baseCollateralVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  99,
                  111,
                  108,
                  108,
                  97,
                  116,
                  101,
                  114,
                  97,
                  108
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "baseMint"
              }
            ]
          }
        },
        {
          "name": "quoteCollateralVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  99,
                  111,
                  108,
                  108,
                  97,
                  116,
                  101,
                  114,
                  97,
                  108
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "quoteMint"
              }
            ]
          }
        },
        {
          "name": "baseInsuranceVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "baseMint"
              }
            ]
          }
        },
        {
          "name": "quoteInsuranceVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "quoteMint"
              }
            ]
          }
        },
        {
          "name": "baseFeeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  102,
                  101,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "baseMint"
              }
            ]
          }
        },
        {
          "name": "quoteFeeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  102,
                  101,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "quoteMint"
              }
            ]
          }
        },
        {
          "name": "baseStakeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  115,
                  116,
                  97,
                  107,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "baseClaimTokenMint"
              }
            ]
          }
        },
        {
          "name": "quoteStakeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  115,
                  116,
                  97,
                  107,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "quoteClaimTokenMint"
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "initializeMarketArgs"
            }
          }
        }
      ]
    },
    {
      "name": "liquidate",
      "discriminator": [
        223,
        179,
        226,
        125,
        48,
        46,
        39,
        74
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "liquidator",
          "writable": true,
          "signer": true
        },
        {
          "name": "debtAssetMint"
        },
        {
          "name": "collateralAssetMint"
        },
        {
          "name": "reserveVault",
          "writable": true
        },
        {
          "name": "collateralVault",
          "writable": true
        },
        {
          "name": "insuranceVault",
          "writable": true
        },
        {
          "name": "liquidatorDebtAccount",
          "writable": true
        },
        {
          "name": "liquidatorCollateralAccount",
          "writable": true
        },
        {
          "name": "marginPosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  103,
                  105,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "margin_position.owner",
                "account": "marginPosition"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "liquidateArgs"
            }
          }
        }
      ]
    },
    {
      "name": "openHedge",
      "discriminator": [
        76,
        209,
        98,
        107,
        64,
        37,
        197,
        168
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "claimTokenMint"
        },
        {
          "name": "hedgeTokenMint",
          "writable": true
        },
        {
          "name": "hedgeVault",
          "writable": true
        },
        {
          "name": "ownerClaimAccount",
          "writable": true
        },
        {
          "name": "ownerHedgeAccount",
          "writable": true
        },
        {
          "name": "hedgePosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  101,
                  100,
                  103,
                  101,
                  95,
                  112,
                  111,
                  115,
                  105,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "owner"
              },
              {
                "kind": "account",
                "path": "assetMint"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "openHedgeArgs"
            }
          }
        }
      ]
    },
    {
      "name": "removeLiquidity",
      "discriminator": [
        80,
        85,
        209,
        72,
        24,
        206,
        177,
        108
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "claimTokenMint",
          "writable": true
        },
        {
          "name": "reserveVault",
          "writable": true
        },
        {
          "name": "ownerAssetAccount",
          "writable": true
        },
        {
          "name": "ownerClaimAccount",
          "writable": true
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "removeLiquidityArgs"
            }
          }
        }
      ]
    },
    {
      "name": "repay",
      "discriminator": [
        234,
        103,
        67,
        82,
        208,
        234,
        219,
        166
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "debtAssetMint"
        },
        {
          "name": "reserveVault",
          "writable": true
        },
        {
          "name": "ownerDebtAccount",
          "writable": true
        },
        {
          "name": "marginPosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  103,
                  105,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "owner"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "repayArgs"
            }
          }
        }
      ]
    },
    {
      "name": "setReduceOnly",
      "discriminator": [
        187,
        233,
        208,
        249,
        160,
        104,
        209,
        117
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "operator",
          "signer": true
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "setMarketReduceOnlyArgs"
            }
          }
        }
      ]
    },
    {
      "name": "stake",
      "discriminator": [
        206,
        176,
        202,
        18,
        200,
        209,
        179,
        108
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "claimTokenMint"
        },
        {
          "name": "stakeVault",
          "writable": true
        },
        {
          "name": "ownerClaimAccount",
          "writable": true
        },
        {
          "name": "stakePosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  116,
                  97,
                  107,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "owner"
              },
              {
                "kind": "account",
                "path": "assetMint"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "stakeArgs"
            }
          }
        }
      ]
    },
    {
      "name": "swap",
      "discriminator": [
        248,
        198,
        158,
        145,
        225,
        117,
        135,
        200
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "trader",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetInMint"
        },
        {
          "name": "assetOutMint"
        },
        {
          "name": "reserveInVault",
          "writable": true
        },
        {
          "name": "reserveOutVault",
          "writable": true
        },
        {
          "name": "feeInVault",
          "writable": true
        },
        {
          "name": "traderAssetInAccount",
          "writable": true
        },
        {
          "name": "traderAssetOutAccount",
          "writable": true
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "swapArgs"
            }
          }
        }
      ]
    },
    {
      "name": "unstake",
      "discriminator": [
        90,
        95,
        107,
        42,
        205,
        124,
        50,
        225
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "claimTokenMint"
        },
        {
          "name": "stakeVault",
          "writable": true
        },
        {
          "name": "ownerClaimAccount",
          "writable": true
        },
        {
          "name": "stakePosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  116,
                  97,
                  107,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "owner"
              },
              {
                "kind": "account",
                "path": "assetMint"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "unstakeArgs"
            }
          }
        }
      ]
    },
    {
      "name": "updateConfig",
      "discriminator": [
        29,
        158,
        252,
        191,
        10,
        83,
        219,
        99
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "operator",
          "signer": true
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "updateMarketConfigArgs"
            }
          }
        }
      ]
    },
    {
      "name": "withdrawCollateral",
      "discriminator": [
        115,
        135,
        168,
        106,
        139,
        214,
        138,
        150
      ],
      "accounts": [
        {
          "name": "market",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  95,
                  118,
                  50
                ]
              },
              {
                "kind": "account",
                "path": "market.base_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.quote_mint",
                "account": "market"
              },
              {
                "kind": "account",
                "path": "market.params_hash",
                "account": "market"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "assetMint"
        },
        {
          "name": "collateralVault",
          "writable": true
        },
        {
          "name": "ownerAssetAccount",
          "writable": true
        },
        {
          "name": "marginPosition",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  103,
                  105,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "market"
              },
              {
                "kind": "account",
                "path": "owner"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "eventAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "withdrawCollateralArgs"
            }
          }
        }
      ]
    }
  ],
  "accounts": [
    {
      "name": "hedgePosition",
      "discriminator": [
        88,
        83,
        42,
        238,
        125,
        99,
        252,
        52
      ]
    },
    {
      "name": "marginPosition",
      "discriminator": [
        176,
        105,
        128,
        156,
        137,
        230,
        126,
        186
      ]
    },
    {
      "name": "market",
      "discriminator": [
        219,
        190,
        213,
        55,
        0,
        227,
        198,
        154
      ]
    },
    {
      "name": "stakePosition",
      "discriminator": [
        78,
        165,
        30,
        111,
        171,
        125,
        11,
        220
      ]
    }
  ],
  "events": [
    {
      "name": "liquidityAdded",
      "discriminator": [
        154,
        26,
        221,
        108,
        238,
        64,
        217,
        161
      ]
    },
    {
      "name": "liquidityRemoved",
      "discriminator": [
        225,
        105,
        216,
        39,
        124,
        116,
        169,
        189
      ]
    },
    {
      "name": "marketCollateralDeposited",
      "discriminator": [
        41,
        53,
        157,
        172,
        249,
        159,
        63,
        60
      ]
    },
    {
      "name": "marketCollateralWithdrawn",
      "discriminator": [
        68,
        208,
        162,
        132,
        39,
        151,
        221,
        245
      ]
    },
    {
      "name": "marketCreated",
      "discriminator": [
        88,
        184,
        130,
        231,
        226,
        84,
        6,
        58
      ]
    },
    {
      "name": "marketDebtUpdated",
      "discriminator": [
        135,
        150,
        109,
        165,
        174,
        35,
        163,
        151
      ]
    },
    {
      "name": "marketFeeLiabilityClaimed",
      "discriminator": [
        8,
        222,
        222,
        67,
        44,
        111,
        218,
        8
      ]
    },
    {
      "name": "marketFeesClaimed",
      "discriminator": [
        216,
        66,
        148,
        204,
        52,
        7,
        196,
        0
      ]
    },
    {
      "name": "marketHealthUpdated",
      "discriminator": [
        99,
        12,
        230,
        43,
        133,
        194,
        188,
        225
      ]
    },
    {
      "name": "marketHedgeClosed",
      "discriminator": [
        94,
        23,
        216,
        248,
        238,
        103,
        249,
        140
      ]
    },
    {
      "name": "marketHedgeFeesClaimed",
      "discriminator": [
        107,
        188,
        240,
        134,
        159,
        15,
        240,
        43
      ]
    },
    {
      "name": "marketHedgeOpened",
      "discriminator": [
        234,
        65,
        71,
        203,
        229,
        161,
        238,
        22
      ]
    },
    {
      "name": "marketInsuranceFunded",
      "discriminator": [
        173,
        170,
        246,
        1,
        232,
        8,
        182,
        16
      ]
    },
    {
      "name": "marketStakeUpdated",
      "discriminator": [
        63,
        209,
        17,
        74,
        217,
        206,
        214,
        193
      ]
    },
    {
      "name": "marketUpdated",
      "discriminator": [
        170,
        51,
        74,
        147,
        116,
        168,
        217,
        251
      ]
    },
    {
      "name": "positionLiquidated",
      "discriminator": [
        40,
        107,
        90,
        214,
        96,
        30,
        61,
        128
      ]
    },
    {
      "name": "swapExecuted",
      "discriminator": [
        150,
        166,
        26,
        225,
        28,
        89,
        38,
        79
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "invalidDeployer",
      "msg": "Invalid deployer"
    },
    {
      "code": 6001,
      "name": "argumentMissing",
      "msg": "Argument missing"
    },
    {
      "code": 6002,
      "name": "invalidSwapFeeBps",
      "msg": "Invalid swap fee bps"
    },
    {
      "code": 6003,
      "name": "invalidInterestFeeBps",
      "msg": "Invalid interest fee bps"
    },
    {
      "code": 6004,
      "name": "invalidHalfLife",
      "msg": "Invalid half life"
    },
    {
      "code": 6005,
      "name": "invalidFutarchyAuthority",
      "msg": "Invalid futarchy authority"
    },
    {
      "code": 6006,
      "name": "invalidReduceOnlyAuthority",
      "msg": "Invalid reduce-only authority"
    },
    {
      "code": 6007,
      "name": "invalidArgument",
      "msg": "Invalid argument"
    },
    {
      "code": 6008,
      "name": "amountZero",
      "msg": "Amount cannot be zero"
    },
    {
      "code": 6009,
      "name": "insufficientAmount0In",
      "msg": "Insufficient amount0 in"
    },
    {
      "code": 6010,
      "name": "insufficientAmount1In",
      "msg": "Insufficient amount1 in"
    },
    {
      "code": 6011,
      "name": "borrowingPowerExceeded",
      "msg": "Borrowing power exceeded"
    },
    {
      "code": 6012,
      "name": "invalidTokenAccount",
      "msg": "Invalid token account"
    },
    {
      "code": 6013,
      "name": "invalidTokenProgram",
      "msg": "Invalid token program"
    },
    {
      "code": 6014,
      "name": "borrowExceedsReserve",
      "msg": "Borrow exceeds reserve"
    },
    {
      "code": 6015,
      "name": "insufficientAmount0",
      "msg": "Insufficient amount0"
    },
    {
      "code": 6016,
      "name": "insufficientAmount1",
      "msg": "Insufficient amount1"
    },
    {
      "code": 6017,
      "name": "insufficientOutputAmount",
      "msg": "Insufficient output amount"
    },
    {
      "code": 6018,
      "name": "slippageExceeded",
      "msg": "Output amount below minimum requested (slippage exceeded)"
    },
    {
      "code": 6019,
      "name": "insufficientLiquidity",
      "msg": "Insufficient liquidity"
    },
    {
      "code": 6020,
      "name": "insufficientCashReserve0",
      "msg": "Insufficient cash reserve0"
    },
    {
      "code": 6021,
      "name": "insufficientCashReserve1",
      "msg": "Insufficient cash reserve1"
    },
    {
      "code": 6022,
      "name": "overflow",
      "msg": "Arithmetic overflow"
    },
    {
      "code": 6023,
      "name": "undercollateralized",
      "msg": "undercollateralized"
    },
    {
      "code": 6024,
      "name": "insufficientBalanceForCollateral",
      "msg": "Insufficient balance for collateral"
    },
    {
      "code": 6025,
      "name": "insufficientAmount",
      "msg": "Insufficient amount"
    },
    {
      "code": 6026,
      "name": "insufficientBalance",
      "msg": "User balance insufficient to cover requested amount"
    },
    {
      "code": 6027,
      "name": "insufficientDebt",
      "msg": "Insufficient debt"
    },
    {
      "code": 6028,
      "name": "userPositionNotInitialized",
      "msg": "User position not initialized"
    },
    {
      "code": 6029,
      "name": "zeroDebtAmount",
      "msg": "Zero debt amount"
    },
    {
      "code": 6030,
      "name": "notUndercollateralized",
      "msg": "Not undercollateralized"
    },
    {
      "code": 6031,
      "name": "brokenInvariant",
      "msg": "Broken invariant"
    },
    {
      "code": 6032,
      "name": "invariantOverflow",
      "msg": "Math overflow during invariant calculation"
    },
    {
      "code": 6033,
      "name": "feeMathOverflow",
      "msg": "Math overflow during fee calculation."
    },
    {
      "code": 6034,
      "name": "outputAmountOverflow",
      "msg": "Math overflow during output amount calculation."
    },
    {
      "code": 6035,
      "name": "reserveOverflow",
      "msg": "Math overflow during reserve calculation."
    },
    {
      "code": 6036,
      "name": "reserveUnderflow",
      "msg": "Math underflow during reserve calculation."
    },
    {
      "code": 6037,
      "name": "cashReserveUnderflow",
      "msg": "Math underflow during cash reserve calculation."
    },
    {
      "code": 6038,
      "name": "denominatorOverflow",
      "msg": "Math overflow during denominator calculation."
    },
    {
      "code": 6039,
      "name": "liquidityMathOverflow",
      "msg": "Math overflow during liquidity calculation"
    },
    {
      "code": 6040,
      "name": "liquiditySqrtOverflow",
      "msg": "Math overflow during liquidity square root calculation"
    },
    {
      "code": 6041,
      "name": "liquidityUnderflow",
      "msg": "Math underflow during liquidity calculation"
    },
    {
      "code": 6042,
      "name": "liquidityConversionOverflow",
      "msg": "Math overflow during liquidity conversion"
    },
    {
      "code": 6043,
      "name": "supplyOverflow",
      "msg": "Math overflow during supply calculation"
    },
    {
      "code": 6044,
      "name": "supplyUnderflow",
      "msg": "Math underflow during supply calculation"
    },
    {
      "code": 6045,
      "name": "debtMathOverflow",
      "msg": "Math overflow during debt calculation"
    },
    {
      "code": 6046,
      "name": "debtShareMathOverflow",
      "msg": "Math overflow during debt share calculation"
    },
    {
      "code": 6047,
      "name": "debtShareDivisionOverflow",
      "msg": "Math overflow during debt share division"
    },
    {
      "code": 6048,
      "name": "debtUtilizationOverflow",
      "msg": "Math overflow during debt utilization calculation"
    },
    {
      "code": 6049,
      "name": "invalidMint",
      "msg": "Invalid mint"
    },
    {
      "code": 6050,
      "name": "invalidMintLen",
      "msg": "Invalid mint length"
    },
    {
      "code": 6051,
      "name": "invalidDistribution",
      "msg": "Invalid distribution - percentages must sum to 100%"
    },
    {
      "code": 6052,
      "name": "invalidLpMintKey",
      "msg": "Invalid LP mint key"
    },
    {
      "code": 6053,
      "name": "invalidLpName",
      "msg": "Invalid LP name"
    },
    {
      "code": 6054,
      "name": "invalidLpSymbol",
      "msg": "Invalid LP symbol"
    },
    {
      "code": 6055,
      "name": "invalidLpUri",
      "msg": "Invalid LP URI"
    },
    {
      "code": 6056,
      "name": "accountNotEmpty",
      "msg": "Account not empty"
    },
    {
      "code": 6057,
      "name": "invalidMintAuthority",
      "msg": "Invalid mint authority"
    },
    {
      "code": 6058,
      "name": "frozenLpMint",
      "msg": "Frozen LP mint"
    },
    {
      "code": 6059,
      "name": "nonZeroSupply",
      "msg": "Non-zero supply"
    },
    {
      "code": 6060,
      "name": "wrongLpDecimals",
      "msg": "Wrong LP decimals"
    },
    {
      "code": 6061,
      "name": "invalidVaultSameAccount",
      "msg": "Invalid vault - token_in_vault and token_out_vault must be different"
    },
    {
      "code": 6062,
      "name": "invalidVault",
      "msg": "Invalid vault"
    },
    {
      "code": 6063,
      "name": "invalidParamsHash",
      "msg": "Invalid params hash - hash does not match computed parameters"
    },
    {
      "code": 6064,
      "name": "invalidVersion",
      "msg": "Invalid version"
    },
    {
      "code": 6065,
      "name": "invalidTokenOrder",
      "msg": "Invalid token order"
    },
    {
      "code": 6066,
      "name": "invalidRateModel",
      "msg": "Invalid rate model - rate_model does not match market configuration"
    },
    {
      "code": 6067,
      "name": "invalidPositionMarket",
      "msg": "Invalid position market - position does not match market"
    },
    {
      "code": 6068,
      "name": "invalidUtilBounds",
      "msg": "Invalid utilization bounds - must satisfy: MIN <= start < end <= MAX"
    },
    {
      "code": 6069,
      "name": "invalidRateParams",
      "msg": "Invalid rate parameters - check half_life_ms, min_rate_bps, max_rate_bps, initial_rate_bps bounds"
    },
    {
      "code": 6070,
      "name": "reduceOnlyMode",
      "msg": "Operation blocked: reduce-only mode is active"
    },
    {
      "code": 6071,
      "name": "reduceOnlyHasDebt",
      "msg": "Cannot remove collateral in reduce-only mode while debt exists"
    },
    {
      "code": 6072,
      "name": "liquidityDeltaCircuitBreaker",
      "msg": "Operation blocked: same-transaction liquidity delta detected"
    },
    {
      "code": 6073,
      "name": "liquidityDeltaCircuitBreakerCpi",
      "msg": "Operation blocked: liquidity delta instruction must be top-level"
    },
    {
      "code": 6074,
      "name": "invalidInstructionsSysvar",
      "msg": "Invalid instructions sysvar"
    },
    {
      "code": 6075,
      "name": "insufficientPostWithdrawDebtCoverage",
      "msg": "Insufficient post-withdraw debt coverage"
    },
    {
      "code": 6076,
      "name": "invalidRecipient",
      "msg": "Invalid recipient - address does not match configured revenue recipient"
    },
    {
      "code": 6077,
      "name": "invalidMarket",
      "msg": "Invalid market"
    },
    {
      "code": 6078,
      "name": "invalidMarketConfig",
      "msg": "Invalid market config"
    },
    {
      "code": 6079,
      "name": "invalidMarketBufferRatio",
      "msg": "Invalid market buffer ratio"
    },
    {
      "code": 6080,
      "name": "insufficientMarketClaimCoverage",
      "msg": "Market claim coverage is insufficient"
    },
    {
      "code": 6081,
      "name": "invalidMarketSide",
      "msg": "Invalid market side"
    },
    {
      "code": 6082,
      "name": "invalidStakePosition",
      "msg": "Invalid stake position"
    },
    {
      "code": 6083,
      "name": "invalidHedgePosition",
      "msg": "Invalid hedge position"
    },
    {
      "code": 6084,
      "name": "insufficientBufferShares",
      "msg": "Buffer shares are insufficient"
    },
    {
      "code": 6085,
      "name": "insufficientBorrowHeadroom",
      "msg": "Borrow headroom is insufficient"
    },
    {
      "code": 6086,
      "name": "insufficientMarketHealth",
      "msg": "Market health is insufficient"
    },
    {
      "code": 6087,
      "name": "invalidMarginPosition",
      "msg": "Invalid margin position"
    },
    {
      "code": 6088,
      "name": "insufficientRecognizedCollateral",
      "msg": "Recognized collateral is insufficient"
    },
    {
      "code": 6089,
      "name": "positionNotLiquidatable",
      "msg": "Position is not liquidatable"
    },
    {
      "code": 6090,
      "name": "insufficientInsuranceReserve",
      "msg": "Insurance reserve is insufficient"
    },
    {
      "code": 6091,
      "name": "liquidationSocializationExceeded",
      "msg": "Socialized liquidation loss exceeds caller cap"
    },
    {
      "code": 6092,
      "name": "invalidClaimMint",
      "msg": "Claim mint must not charge transfer fees"
    },
    {
      "code": 6093,
      "name": "unbackedFeeLiability",
      "msg": "Fee liability is not backed by fee vault balance"
    },
    {
      "code": 6094,
      "name": "invalidMarketFeeAuthority",
      "msg": "Invalid market fee authority"
    },
    {
      "code": 6095,
      "name": "marketReduceOnly",
      "msg": "Market is reduce-only"
    },
    {
      "code": 6096,
      "name": "marketNotStarted",
      "msg": "Market has not started"
    },
    {
      "code": 6097,
      "name": "marketMathOverflow",
      "msg": "Market math overflow"
    },
    {
      "code": 6098,
      "name": "dailyLimitExceeded",
      "msg": "Daily liquidity limit exceeded"
    },
    {
      "code": 6099,
      "name": "marketRiskCircuitBreaker",
      "msg": "Market risk circuit breaker triggered"
    },
    {
      "code": 6100,
      "name": "instructionNotLive",
      "msg": "Instruction is intentionally not live yet"
    }
  ],
  "types": [
    {
      "name": "addLiquidityArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "depositAmount",
            "type": "u64"
          },
          {
            "name": "minClaimAmount",
            "type": "u64"
          },
          {
            "name": "maxBufferAmount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "borrowArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "borrowAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "borrowAmount",
            "type": "u64"
          },
          {
            "name": "minDebtAmountOut",
            "type": "u64"
          },
          {
            "name": "minHealthBps",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "bufferLedger",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bufferShareSupply",
            "type": "u64"
          },
          {
            "name": "stakedBufferShareAmount",
            "type": "u64"
          },
          {
            "name": "requiredBuffer",
            "type": "u64"
          },
          {
            "name": "bufferRatioBps",
            "type": "u16"
          }
        ]
      }
    },
    {
      "name": "claimFeesArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "minFeeAmount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "claimHedgeFeesArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "minFeeAmount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "claimMarketFeesArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "claimKind",
            "type": {
              "defined": {
                "name": "marketFeeClaimKind"
              }
            }
          },
          {
            "name": "minFeeAmount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "claimTokenLedger",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "protectedClaimTokenSupply",
            "type": "u64"
          },
          {
            "name": "hedgedClaimTokenSupply",
            "type": "u64"
          },
          {
            "name": "stakedClaimTokenSupply",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "closeHedgeArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "hedgeAmount",
            "type": "u64"
          },
          {
            "name": "minClaimAmountOut",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "dailyLimitBook",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "borrowedBucket",
            "type": "u64"
          },
          {
            "name": "withdrawnBucket",
            "type": "u64"
          },
          {
            "name": "lastDecaySlot",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "debtBook",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "fixedBaseDebtShares",
            "type": "u128"
          },
          {
            "name": "fixedQuoteDebtShares",
            "type": "u128"
          },
          {
            "name": "softBaseDebtShares",
            "type": "u128"
          },
          {
            "name": "softQuoteDebtShares",
            "type": "u128"
          },
          {
            "name": "baseBorrowIndexNad",
            "type": "u128"
          },
          {
            "name": "quoteBorrowIndexNad",
            "type": "u128"
          }
        ]
      }
    },
    {
      "name": "depositCollateralArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "depositAmount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "depositInsuranceArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "depositAmount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "feeLedger",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "feeGrowthIndexNad",
            "type": "u128"
          },
          {
            "name": "hedgedFeeGrowthIndexNad",
            "type": "u128"
          },
          {
            "name": "feeVaultBalance",
            "type": "u64"
          },
          {
            "name": "feeLiability",
            "type": "u64"
          },
          {
            "name": "hedgedFeeLiability",
            "type": "u64"
          },
          {
            "name": "unallocatedFeeLiability",
            "type": "u64"
          },
          {
            "name": "unallocatedHedgedFeeLiability",
            "type": "u64"
          },
          {
            "name": "protocolFeeLiability",
            "type": "u64"
          },
          {
            "name": "operatorFeeLiability",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "hedgePosition",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "hedgedClaimTokenAmount",
            "type": "u64"
          },
          {
            "name": "feeGrowthCheckpointNad",
            "type": "u128"
          },
          {
            "name": "accruedFeeAmount",
            "type": "u64"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "initializeMarketArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "operator",
            "type": "pubkey"
          },
          {
            "name": "manager",
            "type": "pubkey"
          },
          {
            "name": "config",
            "type": {
              "defined": {
                "name": "marketConfig"
              }
            }
          },
          {
            "name": "paramsHash",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          }
        ]
      }
    },
    {
      "name": "insuranceReserve",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "baseVault",
            "type": "pubkey"
          },
          {
            "name": "quoteVault",
            "type": "pubkey"
          },
          {
            "name": "baseAvailable",
            "type": "u64"
          },
          {
            "name": "quoteAvailable",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "liquidateArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "debtAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "repayAmount",
            "type": "u64"
          },
          {
            "name": "minCollateralOut",
            "type": "u64"
          },
          {
            "name": "maxInsuranceDraw",
            "type": "u64"
          },
          {
            "name": "maxSocializedLoss",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "liquidityAdded",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "reserveCredit",
            "type": "u64"
          },
          {
            "name": "claimAmount",
            "type": "u64"
          },
          {
            "name": "bufferAmount",
            "type": "u64"
          },
          {
            "name": "protectedClaimTokenSupply",
            "type": "u64"
          },
          {
            "name": "requiredBuffer",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "liquidityRemoved",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "claimAmount",
            "type": "u64"
          },
          {
            "name": "protectedClaimTokenSupply",
            "type": "u64"
          },
          {
            "name": "requiredBuffer",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marginPosition",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "baseCollateral",
            "type": "u64"
          },
          {
            "name": "quoteCollateral",
            "type": "u64"
          },
          {
            "name": "recognizedBaseCollateralForQuoteDebt",
            "type": "u64"
          },
          {
            "name": "recognizedQuoteCollateralForBaseDebt",
            "type": "u64"
          },
          {
            "name": "fixedBaseDebtShares",
            "type": "u128"
          },
          {
            "name": "fixedQuoteDebtShares",
            "type": "u128"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "market",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "version",
            "type": "u8"
          },
          {
            "name": "baseMint",
            "type": "pubkey"
          },
          {
            "name": "quoteMint",
            "type": "pubkey"
          },
          {
            "name": "operator",
            "type": "pubkey"
          },
          {
            "name": "manager",
            "type": "pubkey"
          },
          {
            "name": "baseSide",
            "type": {
              "defined": {
                "name": "marketSide"
              }
            }
          },
          {
            "name": "quoteSide",
            "type": {
              "defined": {
                "name": "marketSide"
              }
            }
          },
          {
            "name": "config",
            "type": {
              "defined": {
                "name": "marketConfig"
              }
            }
          },
          {
            "name": "debtBook",
            "type": {
              "defined": {
                "name": "debtBook"
              }
            }
          },
          {
            "name": "riskBook",
            "type": {
              "defined": {
                "name": "riskBook"
              }
            }
          },
          {
            "name": "health",
            "type": {
              "defined": {
                "name": "marketHealth"
              }
            }
          },
          {
            "name": "recognitionLedger",
            "type": {
              "defined": {
                "name": "recognitionLedger"
              }
            }
          },
          {
            "name": "insuranceReserve",
            "type": {
              "defined": {
                "name": "insuranceReserve"
              }
            }
          },
          {
            "name": "paramsHash",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "lastUpdateSlot",
            "type": "u64"
          },
          {
            "name": "reduceOnly",
            "type": "bool"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "marketAsset",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "base"
          },
          {
            "name": "quote"
          }
        ]
      }
    },
    {
      "name": "marketCollateralDeposited",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "collateralCredit",
            "type": "u64"
          },
          {
            "name": "baseCollateral",
            "type": "u64"
          },
          {
            "name": "quoteCollateral",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketCollateralWithdrawn",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "collateralDebit",
            "type": "u64"
          },
          {
            "name": "assetCredit",
            "type": "u64"
          },
          {
            "name": "baseCollateral",
            "type": "u64"
          },
          {
            "name": "quoteCollateral",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "swapFeeBps",
            "type": "u16"
          },
          {
            "name": "operatorFeeBps",
            "type": "u16"
          },
          {
            "name": "protocolFeeBps",
            "type": "u16"
          },
          {
            "name": "bufferRatioBps",
            "type": "u16"
          },
          {
            "name": "feeRoutingKNad",
            "type": "u64"
          },
          {
            "name": "emaHalfLifeMs",
            "type": "u64"
          },
          {
            "name": "directionalEmaHalfLifeMs",
            "type": "u64"
          },
          {
            "name": "kEmaHalfLifeMs",
            "type": "u64"
          },
          {
            "name": "maxDailyBorrowBps",
            "type": "u16"
          },
          {
            "name": "maxDailyWithdrawBps",
            "type": "u16"
          },
          {
            "name": "spotEmaDivergenceBps",
            "type": "u16"
          },
          {
            "name": "kEmaDrawdownBps",
            "type": "u16"
          },
          {
            "name": "recognizedCollateralCapBps",
            "type": "u16"
          },
          {
            "name": "marketHealthMinBps",
            "type": "u16"
          },
          {
            "name": "effectiveDebtWeightMinBps",
            "type": "u16"
          },
          {
            "name": "effectiveDebtGammaNad",
            "type": "u64"
          },
          {
            "name": "softBorrowEnabled",
            "type": "bool"
          },
          {
            "name": "hedgedLpEnabled",
            "type": "bool"
          },
          {
            "name": "startTime",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "marketCreated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "baseMint",
            "type": "pubkey"
          },
          {
            "name": "quoteMint",
            "type": "pubkey"
          },
          {
            "name": "baseClaimTokenMint",
            "type": "pubkey"
          },
          {
            "name": "quoteClaimTokenMint",
            "type": "pubkey"
          },
          {
            "name": "baseStakeVault",
            "type": "pubkey"
          },
          {
            "name": "quoteStakeVault",
            "type": "pubkey"
          },
          {
            "name": "baseCollateralVault",
            "type": "pubkey"
          },
          {
            "name": "quoteCollateralVault",
            "type": "pubkey"
          },
          {
            "name": "baseInsuranceVault",
            "type": "pubkey"
          },
          {
            "name": "quoteInsuranceVault",
            "type": "pubkey"
          },
          {
            "name": "baseHedgeTokenMint",
            "type": "pubkey"
          },
          {
            "name": "quoteHedgeTokenMint",
            "type": "pubkey"
          },
          {
            "name": "baseHedgeVault",
            "type": "pubkey"
          },
          {
            "name": "quoteHedgeVault",
            "type": "pubkey"
          },
          {
            "name": "operator",
            "type": "pubkey"
          },
          {
            "name": "manager",
            "type": "pubkey"
          },
          {
            "name": "bufferRatioBps",
            "type": "u16"
          },
          {
            "name": "swapFeeBps",
            "type": "u16"
          },
          {
            "name": "protocolFeeBps",
            "type": "u16"
          },
          {
            "name": "paramsHash",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "version",
            "type": "u8"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketDebtUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "debtAssetMint",
            "type": "pubkey"
          },
          {
            "name": "debtDelta",
            "type": "i64"
          },
          {
            "name": "fixedBaseDebt",
            "type": "u128"
          },
          {
            "name": "fixedQuoteDebt",
            "type": "u128"
          },
          {
            "name": "baseDebtHealthBps",
            "type": "u64"
          },
          {
            "name": "quoteDebtHealthBps",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketEventMetadata",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "signer",
            "type": "pubkey"
          },
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "slot",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "marketFeeClaimKind",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "operator"
          },
          {
            "name": "protocol"
          }
        ]
      }
    },
    {
      "name": "marketFeeLiabilityClaimed",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "claimKind",
            "type": "u8"
          },
          {
            "name": "feeAmount",
            "type": "u64"
          },
          {
            "name": "remainingFeeLiability",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketFeesClaimed",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "feeAmount",
            "type": "u64"
          },
          {
            "name": "remainingFeeLiability",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketHealth",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "recognizedBaseCollateralForQuoteDebt",
            "type": "u64"
          },
          {
            "name": "recognizedQuoteCollateralForBaseDebt",
            "type": "u64"
          },
          {
            "name": "effectiveBaseDebtNad",
            "type": "u128"
          },
          {
            "name": "effectiveQuoteDebtNad",
            "type": "u128"
          },
          {
            "name": "baseDebtHealthBps",
            "type": "u64"
          },
          {
            "name": "quoteDebtHealthBps",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "marketHealthUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "recognizedBaseCollateralForQuoteDebt",
            "type": "u64"
          },
          {
            "name": "recognizedQuoteCollateralForBaseDebt",
            "type": "u64"
          },
          {
            "name": "effectiveBaseDebtNad",
            "type": "u128"
          },
          {
            "name": "effectiveQuoteDebtNad",
            "type": "u128"
          },
          {
            "name": "baseDebtHealthBps",
            "type": "u64"
          },
          {
            "name": "quoteDebtHealthBps",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketHedgeClosed",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "hedgeAmount",
            "type": "u64"
          },
          {
            "name": "claimAmount",
            "type": "u64"
          },
          {
            "name": "hedgedClaimTokenSupply",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketHedgeFeesClaimed",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "feeAmount",
            "type": "u64"
          },
          {
            "name": "remainingFeeLiability",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketHedgeOpened",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "claimAmount",
            "type": "u64"
          },
          {
            "name": "hedgeAmount",
            "type": "u64"
          },
          {
            "name": "hedgedClaimTokenSupply",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketInsuranceFunded",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "sponsor",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "insuranceCredit",
            "type": "u64"
          },
          {
            "name": "baseAvailable",
            "type": "u64"
          },
          {
            "name": "quoteAvailable",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketSide",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "assetDecimals",
            "type": "u8"
          },
          {
            "name": "claimTokenMint",
            "type": "pubkey"
          },
          {
            "name": "hedgeTokenMint",
            "type": "pubkey"
          },
          {
            "name": "hedgeVault",
            "type": "pubkey"
          },
          {
            "name": "reserveVault",
            "type": "pubkey"
          },
          {
            "name": "collateralVault",
            "type": "pubkey"
          },
          {
            "name": "feeVault",
            "type": "pubkey"
          },
          {
            "name": "stakeVault",
            "type": "pubkey"
          },
          {
            "name": "reserveLedger",
            "type": {
              "defined": {
                "name": "reserveLedger"
              }
            }
          },
          {
            "name": "claimTokenLedger",
            "type": {
              "defined": {
                "name": "claimTokenLedger"
              }
            }
          },
          {
            "name": "bufferLedger",
            "type": {
              "defined": {
                "name": "bufferLedger"
              }
            }
          },
          {
            "name": "feeLedger",
            "type": {
              "defined": {
                "name": "feeLedger"
              }
            }
          },
          {
            "name": "dailyLimitBook",
            "type": {
              "defined": {
                "name": "dailyLimitBook"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketStakeUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "stakedClaimTokenAmount",
            "type": "u64"
          },
          {
            "name": "stakedBufferShareAmount",
            "type": "u64"
          },
          {
            "name": "activeStakeUnits",
            "type": "u64"
          },
          {
            "name": "accruedFeeAmount",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marketUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "reduceOnly",
            "type": "bool"
          },
          {
            "name": "bufferRatioBps",
            "type": "u16"
          },
          {
            "name": "swapFeeBps",
            "type": "u16"
          },
          {
            "name": "operatorFeeBps",
            "type": "u16"
          },
          {
            "name": "protocolFeeBps",
            "type": "u16"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "openHedgeArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "claimAmount",
            "type": "u64"
          },
          {
            "name": "minHedgeAmount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "positionLiquidated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "borrower",
            "type": "pubkey"
          },
          {
            "name": "liquidator",
            "type": "pubkey"
          },
          {
            "name": "debtAssetMint",
            "type": "pubkey"
          },
          {
            "name": "collateralAssetMint",
            "type": "pubkey"
          },
          {
            "name": "repaidAmount",
            "type": "u64"
          },
          {
            "name": "collateralSeized",
            "type": "u64"
          },
          {
            "name": "insuranceDrawn",
            "type": "u64"
          },
          {
            "name": "socializedLoss",
            "type": "u64"
          },
          {
            "name": "remainingDebt",
            "type": "u128"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "recognitionLedger",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "debtBearingBaseCollateralForQuoteDebt",
            "type": "u64"
          },
          {
            "name": "debtBearingQuoteCollateralForBaseDebt",
            "type": "u64"
          },
          {
            "name": "lastRecognitionSlot",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "removeLiquidityArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "claimAmount",
            "type": "u64"
          },
          {
            "name": "minAssetAmountOut",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "repayArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "repayAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "repayAmount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "reserveLedger",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "liveReserve",
            "type": "u64"
          },
          {
            "name": "cashReserve",
            "type": "u64"
          },
          {
            "name": "reservedLiability",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "riskBook",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "basePriceEmaNad",
            "type": "u64"
          },
          {
            "name": "quotePriceEmaNad",
            "type": "u64"
          },
          {
            "name": "directionalBasePriceEmaNad",
            "type": "u64"
          },
          {
            "name": "directionalQuotePriceEmaNad",
            "type": "u64"
          },
          {
            "name": "cachedSpotBasePriceNad",
            "type": "u64"
          },
          {
            "name": "cachedSpotQuotePriceNad",
            "type": "u64"
          },
          {
            "name": "cachedKNad",
            "type": "u128"
          },
          {
            "name": "cachedLiquidityNad",
            "type": "u128"
          },
          {
            "name": "cachedBaseLiquidityNad",
            "type": "u128"
          },
          {
            "name": "cachedQuoteLiquidityNad",
            "type": "u128"
          },
          {
            "name": "kEma",
            "type": "u128"
          },
          {
            "name": "liquidityEma",
            "type": "u128"
          },
          {
            "name": "baseLiquidityEma",
            "type": "u128"
          },
          {
            "name": "quoteLiquidityEma",
            "type": "u128"
          },
          {
            "name": "lastSnapshotSlot",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "setMarketReduceOnlyArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "reduceOnly",
            "type": "bool"
          }
        ]
      }
    },
    {
      "name": "stakeArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "claimAmount",
            "type": "u64"
          },
          {
            "name": "bufferShareAmount",
            "type": "u64"
          },
          {
            "name": "minActiveStakeUnits",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "stakePosition",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "availableBufferShareAmount",
            "type": "u64"
          },
          {
            "name": "stakedClaimTokenAmount",
            "type": "u64"
          },
          {
            "name": "stakedBufferShareAmount",
            "type": "u64"
          },
          {
            "name": "feeGrowthCheckpointNad",
            "type": "u128"
          },
          {
            "name": "accruedFeeAmount",
            "type": "u64"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "swapArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "assetIn",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "exactAssetIn",
            "type": "u64"
          },
          {
            "name": "minAssetOut",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "swapExecuted",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "trader",
            "type": "pubkey"
          },
          {
            "name": "assetInMint",
            "type": "pubkey"
          },
          {
            "name": "assetOutMint",
            "type": "pubkey"
          },
          {
            "name": "reserveCredit",
            "type": "u64"
          },
          {
            "name": "amountInAfterFee",
            "type": "u64"
          },
          {
            "name": "amountOut",
            "type": "u64"
          },
          {
            "name": "feeCredit",
            "type": "u64"
          },
          {
            "name": "metadata",
            "type": {
              "defined": {
                "name": "marketEventMetadata"
              }
            }
          }
        ]
      }
    },
    {
      "name": "unstakeArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "claimAmount",
            "type": "u64"
          },
          {
            "name": "bufferShareAmount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "updateMarketConfigArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "config",
            "type": {
              "defined": {
                "name": "marketConfig"
              }
            }
          }
        ]
      }
    },
    {
      "name": "withdrawCollateralArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "withdrawAmount",
            "type": "u64"
          },
          {
            "name": "minAssetAmountOut",
            "type": "u64"
          }
        ]
      }
    }
  ],
  "constants": [
    {
      "name": "bpsDenominator",
      "type": "u16",
      "value": "10000"
    },
    {
      "name": "hedgePositionSeedPrefix",
      "type": "bytes",
      "value": "[104, 101, 100, 103, 101, 95, 112, 111, 115, 105, 116, 105, 111, 110]"
    },
    {
      "name": "hedgeVaultSeedPrefix",
      "type": "bytes",
      "value": "[104, 101, 100, 103, 101, 100]"
    },
    {
      "name": "insuranceReserveSeedPrefix",
      "type": "bytes",
      "value": "[105, 110, 115, 117, 114, 97, 110, 99, 101]"
    },
    {
      "name": "liquidationIncentiveBps",
      "type": "u16",
      "value": "50"
    },
    {
      "name": "marginPositionSeedPrefix",
      "type": "bytes",
      "value": "[109, 97, 114, 103, 105, 110]"
    },
    {
      "name": "marketCollateralVaultSeedPrefix",
      "type": "bytes",
      "value": "[109, 97, 114, 107, 101, 116, 95, 99, 111, 108, 108, 97, 116, 101, 114, 97, 108]"
    },
    {
      "name": "marketFeeVaultSeedPrefix",
      "type": "bytes",
      "value": "[109, 97, 114, 107, 101, 116, 95, 102, 101, 101]"
    },
    {
      "name": "marketReserveVaultSeedPrefix",
      "type": "bytes",
      "value": "[109, 97, 114, 107, 101, 116, 95, 114, 101, 115, 101, 114, 118, 101]"
    },
    {
      "name": "marketStakeVaultSeedPrefix",
      "type": "bytes",
      "value": "[109, 97, 114, 107, 101, 116, 95, 115, 116, 97, 107, 101]"
    },
    {
      "name": "marketV2SeedPrefix",
      "type": "bytes",
      "value": "[109, 97, 114, 107, 101, 116, 95, 118, 50]"
    },
    {
      "name": "marketVersion",
      "type": "u8",
      "value": "2"
    },
    {
      "name": "nad",
      "docs": [
        "NAD: nine-decimal fixed point unit, similar to WAD in EVM systems."
      ],
      "type": "u64",
      "value": "1000000000"
    },
    {
      "name": "nadDecimals",
      "type": "u8",
      "value": "9"
    },
    {
      "name": "stakePositionSeedPrefix",
      "type": "bytes",
      "value": "[115, 116, 97, 107, 101]"
    },
    {
      "name": "targetMsPerSlot",
      "type": "u64",
      "value": "400"
    }
  ]
};
