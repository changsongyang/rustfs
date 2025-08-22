# RustFS Performance Tuning Guide

This guide explains how to configure RustFS for optimal write performance.

## Quick Start

For maximum write performance, use these settings:

```bash
# Disable scanner completely
export RUSTFS_SCANNER_MODE=disabled

# Or use low-load mode (recommended)
export RUSTFS_SCANNER_MODE=low_load_only

# Start RustFS
./scripts/run-optimized.sh
```

## Configuration Options

### Scanner Configuration

The scanner is responsible for data integrity checks and healing, but can impact write performance.

#### RUSTFS_SCANNER_MODE

Controls the scanner operation mode:

- `disabled` - Scanner is completely disabled (best write performance)
- `low_load_only` - Scanner only runs when system load is low (recommended)
- `normal` - Regular scanning schedule
- `aggressive` - Intensive scanning for data recovery

```bash
export RUSTFS_SCANNER_MODE=low_load_only
```

#### RUSTFS_SCANNER_INTERVAL

Time between scan cycles in seconds (default: 3600 = 1 hour):

```bash
export RUSTFS_SCANNER_INTERVAL=7200  # Scan every 2 hours
```

#### RUSTFS_SCANNER_WRITE_THRESHOLD

Write IOPS threshold to pause scanner (default: 1000):

```bash
export RUSTFS_SCANNER_WRITE_THRESHOLD=500  # Pause scanner when IOPS > 500
```

### Write Buffer Pool Configuration

The write buffer pool optimizes small file writes by batching them in memory.

#### RUSTFS_WRITE_BUFFER_POOL

Enable/disable write buffer pool (default: true):

```bash
export RUSTFS_WRITE_BUFFER_POOL=true
```

#### RUSTFS_WRITE_BUFFER_SIZE_MB

Buffer pool size in megabytes (default: 16):

```bash
export RUSTFS_WRITE_BUFFER_SIZE_MB=32  # Use 32MB buffer
```

## Usage Examples

### 1. Using Environment Variables

```bash
export RUSTFS_VOLUMES=/data/rustfs
export RUSTFS_ADDRESS=:9001
export RUSTFS_ACCESS_KEY=rustfsadmin
export RUSTFS_SECRET_KEY=rustfsadmin
export RUSTFS_SCANNER_MODE=low_load_only
export RUSTFS_SCANNER_INTERVAL=7200
export RUSTFS_WRITE_BUFFER_POOL=true
export RUSTFS_WRITE_BUFFER_SIZE_MB=32

cargo run --release --bin rustfs
```

### 2. Using Command Line Arguments

```bash
cargo run --release --bin rustfs -- \
    --volumes /data/rustfs \
    --address :9001 \
    --access-key rustfsadmin \
    --secret-key rustfsadmin \
    --scanner-mode low_load_only \
    --scanner-interval 7200 \
    --write-buffer-pool true \
    --write-buffer-size-mb 32
```

### 3. Using Configuration File

Create a `.env` file:

```bash
cp deploy/config/rustfs-optimized.env .env
# Edit .env as needed
source .env
cargo run --release --bin rustfs
```

### 4. Using Docker Compose

```bash
docker-compose -f docker-compose-optimized.yml up -d
```

### 5. Using the Optimized Run Script

```bash
./scripts/run-optimized.sh
```

## Performance Profiles

### Write-Heavy Workload

For applications with heavy write operations:

```bash
RUSTFS_SCANNER_MODE=disabled           # No scanning
RUSTFS_WRITE_BUFFER_POOL=true         # Enable buffering
RUSTFS_WRITE_BUFFER_SIZE_MB=64        # Large buffer
```

### Balanced Workload

For mixed read/write operations:

```bash
RUSTFS_SCANNER_MODE=low_load_only     # Smart scanning
RUSTFS_SCANNER_INTERVAL=3600          # Regular interval
RUSTFS_SCANNER_WRITE_THRESHOLD=1000   # Moderate threshold
RUSTFS_WRITE_BUFFER_POOL=true         # Enable buffering
RUSTFS_WRITE_BUFFER_SIZE_MB=32        # Medium buffer
```

### Data Integrity Priority

When data integrity is critical:

```bash
RUSTFS_SCANNER_MODE=normal            # Regular scanning
RUSTFS_SCANNER_INTERVAL=1800          # Frequent scans
RUSTFS_WRITE_BUFFER_POOL=true         # Still use buffering
RUSTFS_WRITE_BUFFER_SIZE_MB=16        # Smaller buffer
```

## Monitoring Performance

After applying optimizations, monitor performance using:

```bash
# Run warp benchmark
warp mixed --host=127.0.0.1:9001 \
    --access-key=rustfsadmin \
    --secret-key=rustfsadmin \
    --duration=3m \
    --obj.size=4KiB

# Expected improvements:
# - PUT operations: 2-3x faster with scanner disabled
# - Overall QPS: 50-70% improvement
# - Reduced latency for small files
```

## Troubleshooting

### Scanner Still Running

If scanner is still impacting performance:

1. Check current configuration:
   ```bash
   ps aux | grep rustfs
   # Look for scanner-related parameters
   ```

2. Verify environment variables:
   ```bash
   env | grep RUSTFS_SCANNER
   ```

3. Completely disable scanner:
   ```bash
   export RUSTFS_SCANNER_MODE=disabled
   ```

### Write Performance Not Improved

1. Check write buffer pool is enabled:
   ```bash
   env | grep RUSTFS_WRITE_BUFFER
   ```

2. Increase buffer size for small files:
   ```bash
   export RUSTFS_WRITE_BUFFER_SIZE_MB=64
   ```

3. Monitor system resources:
   ```bash
   iostat -x 1
   top -H
   ```

## Best Practices

1. **Start with `low_load_only` mode** - This provides a good balance between performance and data integrity

2. **Monitor your workload** - Use metrics to understand your specific needs

3. **Adjust incrementally** - Make one change at a time and measure the impact

4. **Consider your use case**:
   - High-frequency small writes: Increase buffer pool size
   - Large file uploads: Disable scanner during peak hours
   - Mixed workload: Use low_load_only mode

5. **Regular maintenance** - If scanner is disabled, schedule manual scans during maintenance windows

## Advanced Tuning

For further optimization, consider:

- Adjusting erasure coding parameters
- Tuning network buffer sizes
- Optimizing disk I/O scheduler
- Using dedicated SSDs for metadata

See the main documentation for advanced configuration options.
