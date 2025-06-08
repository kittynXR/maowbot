# AI and Robotics Architecture

## Overview

MaowBot's AI system is designed as a first-class citizen within the pipeline architecture, enabling sophisticated AI-driven interactions and preparing for future VRChat robotics integration where AI personalities control virtual avatars.

## Architecture Philosophy

### AI as Pipeline Actions
Instead of a separate bolt-on AI system, AI capabilities are integrated directly into the event pipeline system. This provides:
- Unified event handling
- Composable AI behaviors
- Easy integration with other actions
- Consistent configuration and monitoring

### Stateful Personality System
AI personalities maintain state across interactions:
- Persistent memory
- Emotional state tracking
- Mood influences on responses
- Activity awareness

## Database Schema

### AI Core Tables

#### AI Providers
```sql
ai_providers
├── provider_id (UUID)
├── name (OpenAI, Anthropic, Local)
├── description
├── api_endpoint
├── capabilities (JSONB)
└── enabled
```

#### AI Models
```sql
ai_models
├── model_id (UUID)
├── provider_id → ai_providers
├── name (gpt-4o, claude-3-5-sonnet, etc)
├── description
├── is_default
├── capabilities (JSONB)
│   ├── function_calling
│   ├── vision
│   ├── streaming
│   └── max_tokens
└── parameters (JSONB)
```

#### AI Configurations
```sql
ai_configurations
├── config_id (UUID)
├── name
├── model_id → ai_models
├── temperature (0.0-2.0)
├── max_tokens
├── system_prompt
├── memory_enabled
├── memory_window_size
└── additional_params (JSONB)
```

### Personality System

#### AI Personalities
```sql
ai_personalities
├── personality_id (UUID)
├── name
├── base_prompt
├── voice_config (JSONB)
│   ├── voice
│   ├── pitch
│   └── rate
├── avatar_config (JSONB)
│   ├── default_avatar
│   └── expressions_enabled
├── personality_traits (JSONB)
│   ├── friendliness (0-1)
│   ├── playfulness (0-1)
│   ├── helpfulness (0-1)
│   └── curiosity (0-1)
└── emotional_ranges (JSONB)
    └── [emotion]: {min, max}
```

#### AI Personality State
```sql
ai_personality_state
├── state_id (UUID)
├── personality_id → ai_personalities
├── current_mood (JSONB)
│   ├── happiness (0-1)
│   ├── energy (0-1)
│   ├── confidence (0-1)
│   └── [custom emotions]
├── current_activity
├── last_interaction
└── context_memory (JSONB)
```

### Memory System

#### AI Memory Store
```sql
ai_memory_store
├── memory_id (UUID)
├── personality_id → ai_personalities
├── memory_type (conversation, fact, preference, event)
├── content
├── embedding (vector) - For similarity search
├── importance (0-1)
├── source_context (JSONB)
├── created_at
├── accessed_at
├── access_count
└── expires_at
```

#### AI Conversation History
```sql
ai_conversation_history
├── conversation_id (UUID)
├── personality_id → ai_personalities
├── user_id → users
├── platform
├── channel
├── messages (JSONB[])
├── summary
├── emotional_trajectory (JSONB)
├── started_at
└── ended_at
```

## Pipeline Actions

### AI Response Actions

#### ai_text_response
Generate contextual text responses.
```json
{
  "personality_id": "maow",
  "include_memory": true,
  "emotion_influence": 0.7,
  "response_style": "casual",
  "max_length": 200
}
```

#### ai_voice_response
Generate spoken responses with emotion.
```json
{
  "personality_id": "maow",
  "voice_emotion": "excited",
  "speed": 1.1,
  "pitch_variance": 0.2,
  "include_sound_effects": true
}
```

#### ai_analyze_input
Analyze user input for context.
```json
{
  "analyze_emotion": true,
  "analyze_intent": true,
  "update_mood": true,
  "extract_entities": true,
  "language_detection": true
}
```

### Memory Actions

#### ai_remember
Store information in long-term memory.
```json
{
  "memory_type": "user_preference",
  "importance": 0.8,
  "auto_expire": false,
  "tags": ["favorite_game", "preferences"],
  "compress_similar": true
}
```

#### ai_recall
Retrieve relevant memories.
```json
{
  "context": "What games do I like?",
  "limit": 5,
  "min_importance": 0.3,
  "memory_types": ["preference", "conversation"],
  "time_decay": true
}
```

#### ai_forget
Remove or expire memories.
```json
{
  "memory_ids": ["uuid1", "uuid2"],
  "forget_type": "soft", // soft = reduce importance, hard = delete
  "reason": "user_requested"
}
```

### Mood/State Actions

#### ai_update_mood
Adjust personality emotional state.
```json
{
  "mood_delta": {
    "happiness": 0.2,
    "energy": -0.1,
    "confidence": 0.1
  },
  "reason": "positive_interaction",
  "decay_rate": 0.05
}
```

#### ai_set_activity
Update what the AI is currently doing.
```json
{
  "activity": "playing_game",
  "activity_data": {
    "game": "Minecraft",
    "with_users": ["user123"]
  }
}
```

## VRChat Robotics Integration

### Avatar Control Actions

#### avatar_set_emotion
Control facial expressions.
```json
{
  "emotion": "happy",
  "intensity": 0.8,
  "duration_ms": 5000,
  "blend_with_current": true,
  "priority": 1
}
```

#### avatar_perform_gesture
Trigger animations and gestures.
```json
{
  "gesture": "wave",
  "target_position": [10, 0, 5],
  "loop": false,
  "blend_time": 500,
  "layer": "upper_body"
}
```

#### avatar_move_to
Navigate in VRChat world.
```json
{
  "position": [10.5, 0, -5.2],
  "look_at": [15.0, 1.5, -5.2],
  "speed": 1.5,
  "animation": "walk",
  "avoid_obstacles": true
}
```

#### avatar_interact
Interact with world objects.
```json
{
  "object_id": "chair_001",
  "interaction_type": "sit",
  "duration": 30000,
  "exit_condition": "user_calls"
}
```

### Emotion Mapping

Personality moods map to avatar expressions:

```javascript
{
  "happiness": {
    "high": ["smile", "laugh", "eyes_closed_smile"],
    "medium": ["content", "slight_smile"],
    "low": ["neutral", "slight_frown"]
  },
  "energy": {
    "high": ["excited", "jump", "dance"],
    "medium": ["idle_active", "look_around"],
    "low": ["idle_tired", "yawn", "stretch"]
  }
}
```

## Example Pipelines

### AI Conversation Pipeline
```yaml
name: ai_conversation
filters:
  - platform_filter: ["twitch-irc", "discord"]
  - message_pattern_filter: 
      patterns: ["@maow", "hey maow"]
actions:
  - ai_analyze_input:
      analyze_emotion: true
      update_mood: true
  - ai_recall:
      context: "{event.message}"
      limit: 3
  - ai_text_response:
      personality_id: "maow"
      emotion_influence: 0.7
  - twitch_message:
      message: "{ai.response}"
  - ai_remember:
      memory_type: "conversation"
      importance: "{ai.interaction_importance}"
```

### VRChat Presence Pipeline
```yaml
name: vrchat_ai_presence
filters:
  - platform_filter: ["vrchat"]
  - event_type_filter: ["avatar.spawned"]
actions:
  - ai_set_activity:
      activity: "vrchat_active"
  - avatar_set_emotion:
      emotion: "happy"
      intensity: 0.7
  - ai_text_response:
      personality_id: "maow"
      response_style: "greeting"
  - vrchat_chatbox:
      message: "{ai.response}"
      duration: 5000
```

### Mood Influence Pipeline
```yaml
name: mood_based_responses
filters:
  - platform_filter: ["twitch-irc"]
actions:
  - ai_get_mood:
      personality_id: "maow"
  - conditional:
      if: "mood.happiness < 0.3"
      then:
        - ai_text_response:
            response_style: "melancholic"
      else:
        - ai_text_response:
            response_style: "cheerful"
```

## Implementation Best Practices

### 1. Memory Management
- Implement importance-based eviction
- Use embeddings for semantic search
- Compress similar memories
- Time-decay old memories

### 2. Mood Systems
- Gradual mood changes
- Environmental influences
- User interaction history
- Circadian rhythm simulation

### 3. Response Generation
- Context window management
- Personality consistency
- Emotion-appropriate language
- Platform-specific formatting

### 4. VRChat Integration
- Network-efficient updates
- Animation blending
- Physics-aware movement
- Social space awareness

## Future Enhancements

### Advanced Capabilities
1. **Multi-modal Understanding**: Process images, audio, and text
2. **Predictive Behavior**: Anticipate user needs
3. **Social Graph**: Understand relationships between users
4. **Learning System**: Improve responses over time

### VRChat Features
1. **Object Manipulation**: Pick up and use items
2. **Social Gestures**: Handshakes, hugs, high-fives
3. **Environmental Awareness**: React to world events
4. **Group Interactions**: Coordinate with multiple users

### Technical Improvements
1. **Vector Database**: Efficient similarity search
2. **Model Fine-tuning**: Personality-specific models
3. **Edge Computing**: Local inference for low latency
4. **Federated Learning**: Privacy-preserving improvements