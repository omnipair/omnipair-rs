#!/usr/bin/env bash

# To load the variables in the .env file
source .env

# Build a verifiable build
solana-verify build --library-name $LIBRARY_NAME -u $DEVNET_NETWORK_URL

# To deploy and verify our contract to the devnet
solana program deploy -u $DEVNET_NETWORK_URL target/deploy/$LIBRARY_NAME.so --with-compute-unit-price 50000 --max-sign-attempts 100 --use-rpc

# To verify the contract on the devnet
solana-verify get-program-hash -u $DEVNET_NETWORK_URL $OMNIPAIR_PROGRAM_ID
solana-verify verify-from-image -e target/deploy/$LIBRARY_NAME.so -i ellipsislabs/hello_world_verifiable_build:latest -p $OMNIPAIR_PROGRAM_ID