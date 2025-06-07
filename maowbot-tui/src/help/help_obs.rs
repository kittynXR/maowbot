pub const OBS_HELP: &str = r#"
### OBS Commands

Connect to and control OBS Studio instances via WebSocket.

#### Instance Management
- `obs configure <instance> [options]` - Configure OBS instance
  - `--host <address>` - Set instance IP address
  - `--port <port>` - Set instance port (default: 4455)
  - `--ssl` / `--no-ssl` - Enable/disable SSL (default: off)
  - `--password <password>` - Set WebSocket password
  - `--no-password` - Disable password authentication

Legacy syntax (still supported):
- `obs instance <number> set ip <address>` - Set instance IP address
- `obs instance <number> set port <port>` - Set instance port (default: 4455)
- `obs instance <number> set ssl on/off` - Enable/disable SSL (default: off)
- `obs instance <number> set password <password>` - Set WebSocket password

#### Connection Management
- `connection start obs <instance_number>` - Connect to OBS instance
- `connection stop obs <instance_number>` - Disconnect from OBS instance
- `connection autostart on obs <instance_number>` - Enable autostart
- `connection autostart off obs <instance_number>` - Disable autostart
- `connection status` - Show all connection statuses including OBS

#### Scene Control
- `obs list scenes` - List all scenes
- `obs select scene <number>` - Switch to scene by number from list

#### Source Control
- `obs list sources` - List all sources
- `obs select source <number>` - Select a source by number from list
- `obs source refresh [number]` - Refresh browser source (selected or by number)
- `obs source hide [number]` - Hide source (selected or by number)
- `obs source show [number]` - Show source (selected or by number)

#### Streaming & Recording
- `obs start stream` - Start streaming
- `obs stop stream` - Stop streaming
- `obs start record` - Start recording
- `obs stop record` - Stop recording
- `obs status` - Show streaming/recording status

#### Other Commands
- `obs version` - Show OBS and WebSocket version

### Examples

```
# Connect to default OBS instance
connection start obs 1

# Configure second OBS instance
obs configure 2 --host 192.168.1.100 --password mypassword
connection start obs 2

# Disable password authentication
obs configure 1 --no-password

# Control scenes and sources
obs list scenes
obs select scene 2
obs list sources
obs source refresh 3
obs source hide

# Start streaming
obs start stream
```

### Default Instances

The system comes pre-configured with two OBS instances:
- Instance 1: 127.0.0.1:4455 (localhost)
- Instance 2: 10.11.11.111:4455

### Notes

- OBS WebSocket must be enabled in OBS Studio (Tools > WebSocket Server Settings)
- Supports OBS Studio v31+ with WebSocket Protocol v5
- Multiple instances can be connected simultaneously
- Source hide/show operations affect visibility in the current scene
"#;