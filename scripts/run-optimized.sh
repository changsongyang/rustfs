#!/bin/bash

# RustFS Optimized Run Script for High Write Performance
# This script runs RustFS with optimized settings for write-heavy workloads

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Starting RustFS with optimized write performance settings...${NC}"

# Default values
VOLUMES="${RUSTFS_VOLUMES:-/tmp/rustfs-data}"
ADDRESS="${RUSTFS_ADDRESS:-:9001}"
ACCESS_KEY="${RUSTFS_ACCESS_KEY:-rustfsadmin}"
SECRET_KEY="${RUSTFS_SECRET_KEY:-rustfsadmin}"

# Scanner optimization settings
SCANNER_MODE="${RUSTFS_SCANNER_MODE:-low_load_only}"
SCANNER_INTERVAL="${RUSTFS_SCANNER_INTERVAL:-7200}"
SCANNER_WRITE_THRESHOLD="${RUSTFS_SCANNER_WRITE_THRESHOLD:-1000}"

# Write buffer pool settings
WRITE_BUFFER_POOL="${RUSTFS_WRITE_BUFFER_POOL:-true}"
WRITE_BUFFER_SIZE_MB="${RUSTFS_WRITE_BUFFER_SIZE_MB:-32}"

# Create data directory if it doesn't exist
mkdir -p "$VOLUMES"

echo -e "${YELLOW}Configuration:${NC}"
echo "  - Data directory: $VOLUMES"
echo "  - Address: $ADDRESS"
echo "  - Scanner mode: $SCANNER_MODE"
echo "  - Scanner interval: ${SCANNER_INTERVAL}s"
echo "  - Write threshold: $SCANNER_WRITE_THRESHOLD IOPS"
echo "  - Write buffer pool: $WRITE_BUFFER_POOL"
echo "  - Buffer size: ${WRITE_BUFFER_SIZE_MB}MB"
echo ""

# Build the command
CMD="cargo run --release --bin rustfs -- \
    --volumes $VOLUMES \
    --address $ADDRESS \
    --access-key $ACCESS_KEY \
    --secret-key $SECRET_KEY \
    --scanner-mode $SCANNER_MODE \
    --scanner-interval $SCANNER_INTERVAL \
    --scanner-write-threshold $SCANNER_WRITE_THRESHOLD \
    --write-buffer-pool $WRITE_BUFFER_POOL \
    --write-buffer-size-mb $WRITE_BUFFER_SIZE_MB"

# Add optional parameters if set
if [ -n "$RUSTFS_CONSOLE_ENABLE" ]; then
    CMD="$CMD --console-enable $RUSTFS_CONSOLE_ENABLE"
fi

if [ -n "$RUSTFS_REGION" ]; then
    CMD="$CMD --region $RUSTFS_REGION"
fi

if [ -n "$RUSTFS_TLS_PATH" ]; then
    CMD="$CMD --tls-path $RUSTFS_TLS_PATH"
fi

echo -e "${GREEN}Running RustFS...${NC}"
echo "Command: $CMD"
echo ""

# Run RustFS
exec $CMD
