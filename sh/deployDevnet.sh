#!/usr/bin/env bash

# To load the variables in the .env file
source .env

# Build a verifiable build
solana-verify build --library-name $LIBRARY_NAME -u $DEVNET_NETWORK_URL

# To deploy and verify our contract to the devnet
solana program deploy -u $DEVNET_NETWORK_URL target/deploy/$LIBRARY_NAME.so --with-compute-unit-price 50000 --max-sign-attempts 100 --use-rpc

# To verify the contract on the devnet
SVB_DOCKER_MEMORY_LIMIT=2g SVB_DOCKER_CPU_LIMIT=2
solana-verify get-program-hash -u $DEVNET_NETWORK_URL $OMNIPAIR_PROGRAM_ID
solana-verify verify-from-image -e target/deploy/$LIBRARY_NAME.so -i ellipsislabs/hello_world_verifiable_build:latest -p $OMNIPAIR_PROGRAM_ID
solana-verify verify-from-repo -u $DEVNET_NETWORK_URL --program-id $OMNIPAIR_PROGRAM_ID https://github.com/$REPO_PATH --commit-hash $COMMIT_HASH --library-name $PROGRAM_LIB_NAME --mount-path $MOUNT_PATH
solana-verify submit-job --program-id $OMNIPAIR_PROGRAM_ID --uploader 31rZbBENnRBmjmUxTkCT1LHp59EMmdmTgmooUyaNh3Gh --url $DEVNET_NETWORK_URL
solana-verify verify-from-repo --program-id $OMNIPAIR_PROGRAM_ID https://github.com/$REPO_PATH --library-name $PROGRAM_LIB_NAME -u $DEVNET_NETWORK_URL