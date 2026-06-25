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
          "name": "futarchyAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
          "name": "owner",
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
          "name": "baseYlpMint",
          "writable": true
        },
        {
          "name": "quoteYlpMint",
          "writable": true
        },
        {
          "name": "baseReserveVault",
          "writable": true
        },
        {
          "name": "quoteReserveVault",
          "writable": true
        },
        {
          "name": "ownerBaseAccount",
          "writable": true
        },
        {
          "name": "ownerQuoteAccount",
          "writable": true
        },
        {
          "name": "ownerBaseYlpAccount",
          "writable": true
        },
        {
          "name": "ownerQuoteYlpAccount",
          "writable": true
        },
        {
          "name": "baseYieldAccount",
          "writable": true
        },
        {
          "name": "quoteYieldAccount",
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
          "name": "futarchyAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
      "name": "claimProtocolFees",
      "discriminator": [
        34,
        142,
        219,
        112,
        109,
        54,
        133,
        23
      ],
      "accounts": [
        {
          "name": "caller",
          "writable": true,
          "signer": true
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
          "name": "futarchyAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
          "name": "baseMint"
        },
        {
          "name": "quoteMint"
        },
        {
          "name": "baseFeeVault",
          "writable": true
        },
        {
          "name": "quoteFeeVault",
          "writable": true
        },
        {
          "name": "futarchyTreasury"
        },
        {
          "name": "buybacksVault"
        },
        {
          "name": "teamTreasury"
        },
        {
          "name": "futarchyTreasuryBaseAccount",
          "writable": true
        },
        {
          "name": "futarchyTreasuryQuoteAccount",
          "writable": true
        },
        {
          "name": "buybacksVaultBaseAccount",
          "writable": true
        },
        {
          "name": "buybacksVaultQuoteAccount",
          "writable": true
        },
        {
          "name": "teamTreasuryBaseAccount",
          "writable": true
        },
        {
          "name": "teamTreasuryQuoteAccount",
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
      "args": []
    },
    {
      "name": "claimYield",
      "discriminator": [
        49,
        74,
        111,
        7,
        186,
        22,
        61,
        165
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
          "name": "lpMint"
        },
        {
          "name": "ownerLpAccount",
          "writable": true
        },
        {
          "name": "feeVault",
          "writable": true
        },
        {
          "name": "interestVault",
          "writable": true
        },
        {
          "name": "recipientAssetAccount",
          "writable": true
        },
        {
          "name": "yieldAccount",
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
              "name": "claimYieldArgs"
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
          "name": "baseMint"
        },
        {
          "name": "quoteMint"
        },
        {
          "name": "baseYlpMint",
          "writable": true
        },
        {
          "name": "quoteYlpMint",
          "writable": true
        },
        {
          "name": "targetHlpMint",
          "writable": true
        },
        {
          "name": "baseReserveVault",
          "writable": true
        },
        {
          "name": "quoteReserveVault",
          "writable": true
        },
        {
          "name": "ownerTargetAccount",
          "writable": true
        },
        {
          "name": "ownerHlpAccount",
          "writable": true
        },
        {
          "name": "hlpBaseYlpAccount",
          "writable": true
        },
        {
          "name": "hlpQuoteYlpAccount",
          "writable": true
        },
        {
          "name": "targetYieldAccount",
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
      "name": "initFutarchyAuthority",
      "discriminator": [
        133,
        110,
        154,
        29,
        240,
        206,
        71,
        100
      ],
      "accounts": [
        {
          "name": "deployer",
          "writable": true,
          "signer": true
        },
        {
          "name": "futarchyAuthority",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
          "name": "programData",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  30,
                  198,
                  73,
                  255,
                  177,
                  239,
                  53,
                  26,
                  189,
                  245,
                  158,
                  226,
                  167,
                  183,
                  246,
                  221,
                  30,
                  28,
                  81,
                  246,
                  125,
                  59,
                  35,
                  168,
                  135,
                  79,
                  228,
                  164,
                  248,
                  149,
                  245,
                  53
                ]
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                2,
                168,
                246,
                145,
                78,
                136,
                161,
                176,
                226,
                16,
                21,
                62,
                247,
                99,
                174,
                43,
                0,
                194,
                185,
                61,
                22,
                193,
                36,
                210,
                192,
                83,
                122,
                16,
                4,
                128,
                0,
                0
              ]
            }
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "initFutarchyAuthorityArgs"
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
          "name": "futarchyAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
          "name": "baseYlpMint"
        },
        {
          "name": "quoteYlpMint"
        },
        {
          "name": "baseHlpMint"
        },
        {
          "name": "quoteHlpMint"
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
          "name": "baseInterestVault",
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
                  105,
                  110,
                  116,
                  101,
                  114,
                  101,
                  115,
                  116
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
          "name": "quoteInterestVault",
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
                  105,
                  110,
                  116,
                  101,
                  114,
                  101,
                  115,
                  116
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
          "name": "teamTreasury"
        },
        {
          "name": "teamTreasuryWsolAccount",
          "writable": true
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
          "name": "collateralInsuranceVault",
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
          "name": "futarchyAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
          "name": "owner",
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
          "name": "baseYlpMint",
          "writable": true
        },
        {
          "name": "quoteYlpMint",
          "writable": true
        },
        {
          "name": "targetHlpMint",
          "writable": true
        },
        {
          "name": "baseReserveVault",
          "writable": true
        },
        {
          "name": "quoteReserveVault",
          "writable": true
        },
        {
          "name": "ownerTargetAccount",
          "writable": true
        },
        {
          "name": "ownerHlpAccount",
          "writable": true
        },
        {
          "name": "hlpBaseYlpAccount",
          "writable": true
        },
        {
          "name": "hlpQuoteYlpAccount",
          "writable": true
        },
        {
          "name": "targetYieldAccount",
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
          "name": "baseMint"
        },
        {
          "name": "quoteMint"
        },
        {
          "name": "baseYlpMint",
          "writable": true
        },
        {
          "name": "quoteYlpMint",
          "writable": true
        },
        {
          "name": "baseReserveVault",
          "writable": true
        },
        {
          "name": "quoteReserveVault",
          "writable": true
        },
        {
          "name": "ownerBaseAccount",
          "writable": true
        },
        {
          "name": "ownerQuoteAccount",
          "writable": true
        },
        {
          "name": "ownerBaseYlpAccount",
          "writable": true
        },
        {
          "name": "ownerQuoteYlpAccount",
          "writable": true
        },
        {
          "name": "baseYieldAccount",
          "writable": true
        },
        {
          "name": "quoteYieldAccount",
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
      "name": "setGlobalReduceOnly",
      "discriminator": [
        242,
        151,
        123,
        139,
        239,
        87,
        249,
        98
      ],
      "accounts": [
        {
          "name": "authoritySigner",
          "writable": true,
          "signer": true,
          "address": "3YL87sTCrHMB6DYKorE9CCN4dL45kZPahoREcMLDY6QV"
        },
        {
          "name": "futarchyAuthority",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "setGlobalReduceOnlyArgs"
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
          "name": "authoritySigner",
          "signer": true,
          "address": "3YL87sTCrHMB6DYKorE9CCN4dL45kZPahoREcMLDY6QV"
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
      "name": "setYieldRecipient",
      "discriminator": [
        178,
        211,
        80,
        10,
        138,
        52,
        188,
        22
      ],
      "accounts": [
        {
          "name": "market",
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
          "name": "yieldAccount",
          "writable": true
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
              "name": "setYieldRecipientArgs"
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
          "name": "futarchyAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
          "name": "futarchyAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
          "name": "authoritySigner",
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
      "name": "updateFutarchyAuthority",
      "discriminator": [
        15,
        196,
        157,
        217,
        113,
        226,
        89,
        25
      ],
      "accounts": [
        {
          "name": "authoritySigner",
          "writable": true,
          "signer": true
        },
        {
          "name": "futarchyAuthority",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "updateFutarchyAuthorityArgs"
            }
          }
        }
      ]
    },
    {
      "name": "updateProtocolRevenue",
      "discriminator": [
        176,
        139,
        131,
        197,
        40,
        225,
        125,
        200
      ],
      "accounts": [
        {
          "name": "authoritySigner",
          "writable": true,
          "signer": true
        },
        {
          "name": "futarchyAuthority",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "updateProtocolRevenueArgs"
            }
          }
        }
      ]
    },
    {
      "name": "updateRevenueRecipients",
      "discriminator": [
        116,
        179,
        137,
        47,
        118,
        167,
        65,
        217
      ],
      "accounts": [
        {
          "name": "authoritySigner",
          "writable": true,
          "signer": true
        },
        {
          "name": "futarchyAuthority",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "updateRevenueRecipientsArgs"
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
          "name": "futarchyAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  117,
                  116,
                  97,
                  114,
                  99,
                  104,
                  121,
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
      "name": "futarchyAuthority",
      "discriminator": [
        175,
        247,
        160,
        182,
        140,
        128,
        211,
        226
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
      "name": "yieldAccount",
      "discriminator": [
        233,
        241,
        119,
        6,
        2,
        14,
        106,
        156
      ]
    }
  ],
  "events": [
    {
      "name": "hlpClosed",
      "discriminator": [
        87,
        126,
        152,
        164,
        162,
        203,
        111,
        235
      ]
    },
    {
      "name": "hlpOpened",
      "discriminator": [
        188,
        231,
        244,
        52,
        5,
        151,
        236,
        84
      ]
    },
    {
      "name": "hlpRebalanced",
      "discriminator": [
        48,
        237,
        118,
        177,
        48,
        168,
        104,
        6
      ]
    },
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
      "name": "protocolFeesClaimed",
      "discriminator": [
        22,
        228,
        205,
        252,
        57,
        17,
        156,
        252
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
    },
    {
      "name": "swapSettled",
      "discriminator": [
        104,
        192,
        63,
        194,
        238,
        236,
        149,
        85
      ]
    },
    {
      "name": "yieldClaimed",
      "discriminator": [
        177,
        201,
        94,
        68,
        19,
        200,
        227,
        27
      ]
    },
    {
      "name": "yieldRecipientUpdated",
      "discriminator": [
        154,
        113,
        25,
        74,
        11,
        107,
        114,
        170
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
      "name": "invalidSettlementPrice",
      "msg": "Invalid settlement price"
    },
    {
      "code": 6080,
      "name": "insufficientMarketShareBacking",
      "msg": "Market reserve share backing is insufficient"
    },
    {
      "code": 6081,
      "name": "invalidMarketSide",
      "msg": "Invalid market side"
    },
    {
      "code": 6082,
      "name": "invalidYieldAccount",
      "msg": "Invalid yield account"
    },
    {
      "code": 6083,
      "name": "invalidHlpVault",
      "msg": "Invalid hLP vault"
    },
    {
      "code": 6084,
      "name": "notEnoughAccounts",
      "msg": "Not enough remaining accounts"
    },
    {
      "code": 6085,
      "name": "hlpSettlementUnavailable",
      "msg": "hLP settlement is unavailable"
    },
    {
      "code": 6086,
      "name": "insufficientBorrowHeadroom",
      "msg": "Borrow headroom is insufficient"
    },
    {
      "code": 6087,
      "name": "insufficientMarketHealth",
      "msg": "Market health is insufficient"
    },
    {
      "code": 6088,
      "name": "invalidMarginPosition",
      "msg": "Invalid margin position"
    },
    {
      "code": 6089,
      "name": "insufficientRecognizedCollateral",
      "msg": "Recognized collateral is insufficient"
    },
    {
      "code": 6090,
      "name": "positionNotLiquidatable",
      "msg": "Position is not liquidatable"
    },
    {
      "code": 6091,
      "name": "insufficientInsurance",
      "msg": "Insurance coverage is insufficient"
    },
    {
      "code": 6092,
      "name": "liquidationSocializationExceeded",
      "msg": "Socialized liquidation loss exceeds caller cap"
    },
    {
      "code": 6093,
      "name": "invalidClaimMint",
      "msg": "Claim mint must not charge transfer fees"
    },
    {
      "code": 6094,
      "name": "unbackedFeeLiability",
      "msg": "Fee liability is not backed by fee vault balance"
    },
    {
      "code": 6095,
      "name": "invalidMarketFeeAuthority",
      "msg": "Invalid market fee authority"
    },
    {
      "code": 6096,
      "name": "marketReduceOnly",
      "msg": "Market is reduce-only"
    },
    {
      "code": 6097,
      "name": "marketNotStarted",
      "msg": "Market has not started"
    },
    {
      "code": 6098,
      "name": "marketMathOverflow",
      "msg": "Market math overflow"
    },
    {
      "code": 6099,
      "name": "dailyLimitExceeded",
      "msg": "Daily liquidity limit exceeded"
    },
    {
      "code": 6100,
      "name": "marketRiskCircuitBreaker",
      "msg": "Market risk circuit breaker triggered"
    },
    {
      "code": 6101,
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
            "name": "baseDepositAmount",
            "type": "u64"
          },
          {
            "name": "quoteDepositAmount",
            "type": "u64"
          },
          {
            "name": "minBaseYlpAmount",
            "type": "u64"
          },
          {
            "name": "minQuoteYlpAmount",
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
      "name": "claimYieldArgs",
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
            "name": "tokenKind",
            "type": {
              "defined": {
                "name": "yieldTokenKind"
              }
            }
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
            "name": "targetAsset",
            "type": {
              "defined": {
                "name": "marketAsset"
              }
            }
          },
          {
            "name": "hlpAmount",
            "type": "u64"
          },
          {
            "name": "minTargetAmountOut",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "dailyLimits",
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
      "name": "debt",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "fixedBaseShares",
            "type": "u128"
          },
          {
            "name": "fixedQuoteShares",
            "type": "u128"
          },
          {
            "name": "softBaseShares",
            "type": "u128"
          },
          {
            "name": "softQuoteShares",
            "type": "u128"
          },
          {
            "name": "baseBorrowIndexNad",
            "type": "u128"
          },
          {
            "name": "quoteBorrowIndexNad",
            "type": "u128"
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
            "name": "lastRecognitionSlot",
            "type": "u64"
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
      "name": "fees",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "swapFeeGrowthIndexNad",
            "type": "u128"
          },
          {
            "name": "interestGrowthIndexNad",
            "type": "u128"
          },
          {
            "name": "swapFeeVaultBalance",
            "type": "u64"
          },
          {
            "name": "interestVaultBalance",
            "type": "u64"
          },
          {
            "name": "swapFeeLiability",
            "type": "u64"
          },
          {
            "name": "interestLiability",
            "type": "u64"
          },
          {
            "name": "unallocatedSwapFeeLiability",
            "type": "u64"
          },
          {
            "name": "unallocatedInterestLiability",
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
      "name": "futarchyAuthority",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "version",
            "type": "u8"
          },
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "recipients",
            "type": {
              "defined": {
                "name": "revenueRecipients"
              }
            }
          },
          {
            "name": "revenueShare",
            "type": {
              "defined": {
                "name": "revenueShare"
              }
            }
          },
          {
            "name": "revenueDistribution",
            "type": {
              "defined": {
                "name": "revenueDistribution"
              }
            }
          },
          {
            "name": "globalReduceOnly",
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
      "name": "hlpClosed",
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
            "name": "hlpAmount",
            "type": "u64"
          },
          {
            "name": "baseYlpAmount",
            "type": "u64"
          },
          {
            "name": "quoteYlpAmount",
            "type": "u64"
          },
          {
            "name": "targetAmountOut",
            "type": "u64"
          },
          {
            "name": "debtRepaid",
            "type": "u64"
          },
          {
            "name": "hlpSupply",
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
      "name": "hlpOpened",
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
            "name": "depositAmount",
            "type": "u64"
          },
          {
            "name": "borrowedAmount",
            "type": "u64"
          },
          {
            "name": "baseYlpAmount",
            "type": "u64"
          },
          {
            "name": "quoteYlpAmount",
            "type": "u64"
          },
          {
            "name": "hlpAmount",
            "type": "u64"
          },
          {
            "name": "hlpSupply",
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
      "name": "hlpRebalanced",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "market",
            "type": "pubkey"
          },
          {
            "name": "targetSide",
            "type": "u8"
          },
          {
            "name": "idealDelta",
            "type": "i128"
          },
          {
            "name": "executedDelta",
            "type": "i128"
          },
          {
            "name": "pendingRebalance",
            "type": "i128"
          },
          {
            "name": "navNad",
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
      "name": "hlpVault",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "targetSide",
            "type": "u8"
          },
          {
            "name": "baseYlpVault",
            "type": "pubkey"
          },
          {
            "name": "quoteYlpVault",
            "type": "pubkey"
          },
          {
            "name": "ylpBaseShares",
            "type": "u64"
          },
          {
            "name": "ylpQuoteShares",
            "type": "u64"
          },
          {
            "name": "debtShares",
            "type": "u128"
          },
          {
            "name": "hlpSupply",
            "type": "u64"
          },
          {
            "name": "pendingRebalance",
            "type": "i128"
          },
          {
            "name": "baseSwapFeeGrowthIndexNad",
            "type": "u128"
          },
          {
            "name": "baseInterestGrowthIndexNad",
            "type": "u128"
          },
          {
            "name": "quoteSwapFeeGrowthIndexNad",
            "type": "u128"
          },
          {
            "name": "quoteInterestGrowthIndexNad",
            "type": "u128"
          },
          {
            "name": "baseSwapFeeCheckpointNad",
            "type": "u128"
          },
          {
            "name": "baseInterestCheckpointNad",
            "type": "u128"
          },
          {
            "name": "quoteSwapFeeCheckpointNad",
            "type": "u128"
          },
          {
            "name": "quoteInterestCheckpointNad",
            "type": "u128"
          },
          {
            "name": "unallocatedBaseSwapFeeAmount",
            "type": "u64"
          },
          {
            "name": "unallocatedBaseInterestAmount",
            "type": "u64"
          },
          {
            "name": "unallocatedQuoteSwapFeeAmount",
            "type": "u64"
          },
          {
            "name": "unallocatedQuoteInterestAmount",
            "type": "u64"
          },
          {
            "name": "lastNavNad",
            "type": "u128"
          },
          {
            "name": "cachedSettlementPriceNad",
            "type": "u128"
          },
          {
            "name": "lastRebalanceSlot",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "initFutarchyAuthorityArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "swapBps",
            "type": "u16"
          },
          {
            "name": "interestBps",
            "type": "u16"
          },
          {
            "name": "futarchyTreasury",
            "type": "pubkey"
          },
          {
            "name": "futarchyTreasuryBps",
            "type": "u16"
          },
          {
            "name": "buybacksVault",
            "type": "pubkey"
          },
          {
            "name": "buybacksVaultBps",
            "type": "u16"
          },
          {
            "name": "teamTreasury",
            "type": "pubkey"
          },
          {
            "name": "teamTreasuryBps",
            "type": "u16"
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
      "name": "insurance",
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
            "name": "baseReserveCredit",
            "type": "u64"
          },
          {
            "name": "quoteReserveCredit",
            "type": "u64"
          },
          {
            "name": "baseYlpAmount",
            "type": "u64"
          },
          {
            "name": "quoteYlpAmount",
            "type": "u64"
          },
          {
            "name": "baseYlpSupply",
            "type": "u64"
          },
          {
            "name": "quoteYlpSupply",
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
            "name": "baseYlpAmount",
            "type": "u64"
          },
          {
            "name": "quoteYlpAmount",
            "type": "u64"
          },
          {
            "name": "baseAmountOut",
            "type": "u64"
          },
          {
            "name": "quoteAmountOut",
            "type": "u64"
          },
          {
            "name": "baseYlpSupply",
            "type": "u64"
          },
          {
            "name": "quoteYlpSupply",
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
            "name": "fixedBaseShares",
            "type": "u128"
          },
          {
            "name": "fixedQuoteShares",
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
            "name": "debt",
            "type": {
              "defined": {
                "name": "debt"
              }
            }
          },
          {
            "name": "baseHlpVault",
            "type": {
              "defined": {
                "name": "hlpVault"
              }
            }
          },
          {
            "name": "quoteHlpVault",
            "type": {
              "defined": {
                "name": "hlpVault"
              }
            }
          },
          {
            "name": "risk",
            "type": {
              "defined": {
                "name": "risk"
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
            "name": "insurance",
            "type": {
              "defined": {
                "name": "insurance"
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
            "name": "targetHlpLeverageBps",
            "type": "u16"
          },
          {
            "name": "settlementDivergenceBps",
            "type": "u16"
          },
          {
            "name": "emergencyExitHaircutBps",
            "type": "u16"
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
            "name": "baseYlpMint",
            "type": "pubkey"
          },
          {
            "name": "quoteYlpMint",
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
            "name": "baseHlpMint",
            "type": "pubkey"
          },
          {
            "name": "quoteHlpMint",
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
            "name": "targetHlpLeverageBps",
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
            "name": "ylpMint",
            "type": "pubkey"
          },
          {
            "name": "hlpMint",
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
            "name": "interestVault",
            "type": "pubkey"
          },
          {
            "name": "reserves",
            "type": {
              "defined": {
                "name": "reserves"
              }
            }
          },
          {
            "name": "shares",
            "type": {
              "defined": {
                "name": "reserveShares"
              }
            }
          },
          {
            "name": "fees",
            "type": {
              "defined": {
                "name": "fees"
              }
            }
          },
          {
            "name": "dailyLimits",
            "type": {
              "defined": {
                "name": "dailyLimits"
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
            "name": "targetHlpLeverageBps",
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
            "name": "targetAsset",
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
            "name": "minHlpAmount",
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
            "name": "collateralToLiquidator",
            "type": "u64"
          },
          {
            "name": "insuranceFunded",
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
      "name": "protocolFeesClaimed",
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
            "name": "futarchyTreasuryBaseAmount",
            "type": "u64"
          },
          {
            "name": "futarchyTreasuryQuoteAmount",
            "type": "u64"
          },
          {
            "name": "buybacksVaultBaseAmount",
            "type": "u64"
          },
          {
            "name": "buybacksVaultQuoteAmount",
            "type": "u64"
          },
          {
            "name": "teamTreasuryBaseAmount",
            "type": "u64"
          },
          {
            "name": "teamTreasuryQuoteAmount",
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
      "name": "removeLiquidityArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "baseYlpAmount",
            "type": "u64"
          },
          {
            "name": "quoteYlpAmount",
            "type": "u64"
          },
          {
            "name": "minBaseAmountOut",
            "type": "u64"
          },
          {
            "name": "minQuoteAmountOut",
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
      "name": "reserveShares",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ylpSupply",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "reserves",
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
      "name": "revenueDistribution",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "futarchyTreasuryBps",
            "type": "u16"
          },
          {
            "name": "buybacksVaultBps",
            "type": "u16"
          },
          {
            "name": "teamTreasuryBps",
            "type": "u16"
          }
        ]
      }
    },
    {
      "name": "revenueRecipients",
      "docs": [
        "Revenue recipient wallet addresses. Recipient token accounts are derived or",
        "validated against these owners when protocol fees are claimed."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "futarchyTreasury",
            "type": "pubkey"
          },
          {
            "name": "buybacksVault",
            "type": "pubkey"
          },
          {
            "name": "teamTreasury",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "revenueShare",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "swapBps",
            "type": "u16"
          },
          {
            "name": "interestBps",
            "type": "u16"
          }
        ]
      }
    },
    {
      "name": "risk",
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
      "name": "setGlobalReduceOnlyArgs",
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
      "name": "setYieldRecipientArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "tokenKind",
            "type": {
              "defined": {
                "name": "yieldTokenKind"
              }
            }
          },
          {
            "name": "recipient",
            "type": "pubkey"
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
            "name": "baseHlpPendingRebalance",
            "type": "i128"
          },
          {
            "name": "quoteHlpPendingRebalance",
            "type": "i128"
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
      "name": "swapSettled",
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
            "name": "assetInSide",
            "type": "u8"
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
            "name": "baseHlpPendingRebalance",
            "type": "i128"
          },
          {
            "name": "quoteHlpPendingRebalance",
            "type": "i128"
          }
        ]
      }
    },
    {
      "name": "updateFutarchyAuthorityArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "newAuthority",
            "type": "pubkey"
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
      "name": "updateProtocolRevenueArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "swapBps",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "interestBps",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "revenueDistribution",
            "type": {
              "option": {
                "defined": {
                  "name": "revenueDistribution"
                }
              }
            }
          }
        ]
      }
    },
    {
      "name": "updateRevenueRecipientsArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "futarchyTreasury",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "buybacksVault",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "teamTreasury",
            "type": {
              "option": "pubkey"
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
    },
    {
      "name": "yieldAccount",
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
            "name": "tokenKind",
            "type": "u8"
          },
          {
            "name": "recipient",
            "type": "pubkey"
          },
          {
            "name": "swapFeeCheckpointNad",
            "type": "u128"
          },
          {
            "name": "interestCheckpointNad",
            "type": "u128"
          },
          {
            "name": "accruedSwapFeeAmount",
            "type": "u64"
          },
          {
            "name": "accruedInterestAmount",
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
      "name": "yieldClaimed",
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
            "name": "tokenKind",
            "type": "u8"
          },
          {
            "name": "recipient",
            "type": "pubkey"
          },
          {
            "name": "swapFeeAmount",
            "type": "u64"
          },
          {
            "name": "interestAmount",
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
      "name": "yieldRecipientUpdated",
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
            "name": "tokenKind",
            "type": "u8"
          },
          {
            "name": "recipient",
            "type": "pubkey"
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
      "name": "yieldTokenKind",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "ylp"
          },
          {
            "name": "hlp"
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
      "name": "futarchyAuthoritySeedPrefix",
      "type": "bytes",
      "value": "[102, 117, 116, 97, 114, 99, 104, 121, 95, 97, 117, 116, 104, 111, 114, 105, 116, 121]"
    },
    {
      "name": "hlpYlpVaultSeedPrefix",
      "type": "bytes",
      "value": "[104, 108, 112, 95, 121, 108, 112, 95, 118, 97, 117, 108, 116]"
    },
    {
      "name": "insuranceSeedPrefix",
      "type": "bytes",
      "value": "[105, 110, 115, 117, 114, 97, 110, 99, 101]"
    },
    {
      "name": "liquidationIncentiveBps",
      "type": "u16",
      "value": "100"
    },
    {
      "name": "liquidationPenaltyBps",
      "type": "u16",
      "value": "300"
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
      "name": "marketCreationFeeLamports",
      "type": "u64",
      "value": "200000000"
    },
    {
      "name": "marketFeeVaultSeedPrefix",
      "type": "bytes",
      "value": "[109, 97, 114, 107, 101, 116, 95, 102, 101, 101]"
    },
    {
      "name": "marketInterestVaultSeedPrefix",
      "type": "bytes",
      "value": "[109, 97, 114, 107, 101, 116, 95, 105, 110, 116, 101, 114, 101, 115, 116]"
    },
    {
      "name": "marketReserveVaultSeedPrefix",
      "type": "bytes",
      "value": "[109, 97, 114, 107, 101, 116, 95, 114, 101, 115, 101, 114, 118, 101]"
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
        "NAD: Nine-decimal fixed point unit (1e9 scaling), similar to WAD (1e18) by Maker."
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
      "name": "targetMsPerSlot",
      "type": "u64",
      "value": "400"
    },
    {
      "name": "yieldAccountSeedPrefix",
      "type": "bytes",
      "value": "[121, 105, 101, 108, 100]"
    }
  ]
};
