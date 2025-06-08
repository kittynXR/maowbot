# Hybrid Chat Logging System

## Overview

MaowBot implements a hybrid approach to chat message handling that combines real-time event processing with optional persistent storage. This design supports both immediate event-driven responses and historical analysis while managing the challenges of high-volume chat environments.

## Architecture

### Key Design Principles

1. **Event-based primary system** - Chat messages are processed as events first
2. **Optional persistence** - Database logging can be enabled/disabled per channel
3. **Configurable retention** - Aggressive cleanup policies to manage storage
4. **Sampling support** - Store only a percentage of messages in high-volume channels
5. **Pre-drop processing** - Run analysis pipelines before data deletion

### Database Schema

#### Chat Messages Table (Partitioned)
```sql
chat_messages
├── message_id (UUID)
├── platform (twitch, discord, vrchat)
├── channel
├── user_id → users
├── message_text
├── timestamp
├── metadata (JSONB)
└── PRIMARY KEY (message_id, timestamp)
```

#### Chat Logging Configuration
```sql
chat_logging_config
├── config_id (UUID)
├── platform
├── channel
├── is_enabled
├── retention_days (default: 30)
├── sampling_rate (0.0-1.0, default: 1.0)
└── pre_drop_pipeline_id → event_pipelines
```

#### Chat Sessions
```sql
chat_sessions
├── session_id (UUID)
├── platform
├── channel
├── user_id → users
├── joined_at
├── left_at
├── session_duration_seconds
└── message_count
```

### Configuration

#### Global Settings (bot_config)
```sql
-- Enable/disable system-wide
chat_logging.enabled = true

-- Default retention period
chat_logging.default_retention_days = 30

-- Sampling rate for new channels
chat_logging.default_sampling_rate = 1.0

-- Batching configuration
chat_logging.batch_size = 100
chat_logging.flush_interval_seconds = 5
chat_logging.max_buffer_size = 1000
```

#### Per-Channel Configuration
```sql
-- Example: High-volume channel with sampling
INSERT INTO chat_logging_config 
(platform, channel, is_enabled, retention_days, sampling_rate)
VALUES ('twitch', 'large_streamer', true, 7, 0.1);

-- Example: Important channel with long retention
INSERT INTO chat_logging_config 
(platform, channel, is_enabled, retention_days, sampling_rate)
VALUES ('discord', 'moderator-chat', true, 90, 1.0);
```

## Features

### 1. Partitioning

Tables are partitioned by month for efficient data management:
- Easy bulk deletion of old data
- Better query performance for time-based queries
- Parallel query execution across partitions

### 2. Sampling

For high-volume channels, sampling reduces storage requirements:
- `sampling_rate = 0.1` stores 10% of messages
- Random sampling maintains statistical validity
- Configurable per channel

### 3. Retention Policies

Automatic cleanup based on retention settings:
- Default: 30 days
- Configurable per channel
- Runs during biweekly maintenance

### 4. Pre-Drop Pipelines

Process data before deletion:
```sql
-- Configure a pipeline to run before dropping data
UPDATE chat_logging_config 
SET pre_drop_pipeline_id = 'analyze_and_archive'
WHERE channel = 'main_chat';
```

Example pipeline actions:
- Generate user activity summaries
- Archive important messages
- Update user analysis scores
- Export to cold storage

### 5. Session Tracking

Track user chat sessions:
- Join/leave times
- Session duration
- Message count per session
- Useful for engagement analytics

## Implementation

### Message Flow

1. **Message Received**: Chat message enters system
2. **Event Processing**: Immediate processing via event pipelines
3. **Logging Check**: Check if logging enabled for channel
4. **Sampling**: Apply sampling rate if < 1.0
5. **Batching**: Add to write buffer
6. **Persistence**: Write to database when batch full or timeout

### Write Path

```rust
// Pseudo-code for message logging
async fn log_message(msg: ChatMessage) {
    // Check if logging enabled
    let config = get_channel_config(&msg.platform, &msg.channel).await?;
    if !config.is_enabled {
        return Ok(());
    }
    
    // Apply sampling
    if config.sampling_rate < 1.0 {
        if random() > config.sampling_rate {
            return Ok(()); // Skip this message
        }
    }
    
    // Add to batch
    batch.push(msg);
    
    // Flush if needed
    if batch.len() >= BATCH_SIZE || last_flush.elapsed() > FLUSH_INTERVAL {
        flush_batch().await?;
    }
}
```

### Maintenance

The biweekly maintenance task handles:
1. **Partition Creation**: Creates partitions for next 2 months
2. **Data Cleanup**: Drops partitions older than retention period
3. **Pre-Drop Processing**: Runs configured pipelines before dropping
4. **User Analysis**: Generates summaries from chat data

## Usage Examples

### Enable Logging for a Channel
```sql
INSERT INTO chat_logging_config 
(platform, channel, is_enabled, retention_days, sampling_rate)
VALUES ('twitch', 'my_channel', true, 30, 1.0)
ON CONFLICT (platform, channel) DO UPDATE
SET is_enabled = true;
```

### Configure High-Volume Channel
```sql
-- 10% sampling, 7-day retention
UPDATE chat_logging_config
SET sampling_rate = 0.1, retention_days = 7
WHERE platform = 'twitch' AND channel = 'popular_streamer';
```

### Query Recent Messages
```sql
-- Get messages from last hour
SELECT * FROM chat_messages
WHERE platform = 'twitch' 
  AND channel = 'my_channel'
  AND timestamp > NOW() - INTERVAL '1 hour'
ORDER BY timestamp DESC;
```

### User Activity Analysis
```sql
-- Messages per user in last 30 days
SELECT 
    u.global_username,
    COUNT(*) as message_count,
    AVG(LENGTH(cm.message_text)) as avg_message_length
FROM chat_messages cm
JOIN users u ON cm.user_id = u.user_id
WHERE cm.timestamp > NOW() - INTERVAL '30 days'
GROUP BY u.user_id, u.global_username
ORDER BY message_count DESC;
```

## Performance Optimization

### Indexes
- BRIN index on timestamp for time-series queries
- B-tree indexes on platform, channel for filtering
- User ID index for user-specific queries

### Query Patterns
```sql
-- Efficient: Uses partition pruning
SELECT * FROM chat_messages
WHERE timestamp >= '2024-06-01' AND timestamp < '2024-07-01';

-- Efficient: Uses indexes
SELECT * FROM chat_messages
WHERE platform = 'twitch' AND channel = 'my_channel'
AND timestamp > NOW() - INTERVAL '1 day';

-- Avoid: Full table scan
SELECT * FROM chat_messages
WHERE message_text LIKE '%hello%';
```

## Scaling Considerations

### For 50,000+ Concurrent Chatters

1. **Aggressive Sampling**: Set sampling_rate to 0.01-0.05 (1-5%)
2. **Short Retention**: 1-7 days for high-volume channels
3. **Separate Database**: Consider dedicated database for chat logs
4. **Read Replicas**: Use read replicas for analytics queries
5. **Batch Size**: Increase batch size to reduce write frequency
6. **Async Processing**: Use background workers for writes

### Storage Estimates

With 50,000 chatters at 1 message/minute:
- Full logging: ~2.1M messages/hour, ~250GB/month
- 5% sampling: ~105K messages/hour, ~12.5GB/month
- 1% sampling: ~21K messages/hour, ~2.5GB/month

## Future Enhancements

1. **Compression**: Implement message text compression
2. **Tiered Storage**: Move old partitions to cheaper storage
3. **Real-time Analytics**: Stream processing for live metrics
4. **Smart Sampling**: Importance-based sampling (mod messages, mentions)
5. **Export Tools**: Automated exports to S3/cloud storage