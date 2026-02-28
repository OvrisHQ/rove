#!/bin/bash
# Generates a dev keypair for local testing. NOT for production.
# Run once after cloning: bash manifest/gen_dev_key.sh
openssl genpkey -algorithm ed25519 -out manifest/dev_private_key.pem
openssl pkey -in manifest/dev_private_key.pem -pubout -outform DER \
  -out manifest/dev_public_key.bin
echo "Dev keypair generated. dev_private_key.pem is in .gitignore."
