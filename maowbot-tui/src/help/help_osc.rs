pub const OSC_HELP_TEXT: &str = r#"OSC (Open Sound Control) Command
=================================

Manage OSC service for VRChat avatar parameter control and chatbox communication.

Basic Commands:
  osc start              Start OSC service
  osc stop               Stop OSC service  
  osc restart            Restart OSC service
  osc status             Show OSC service status and configured destinations
  osc discover           Discover local OSCQuery services
  osc raw                Start raw OSC packet monitor (shows all incoming packets)

Chatbox:
  osc chatbox [message]  Send message to VRChat chatbox
                         If no message provided, enters interactive chatbox mode
                         Type /quit in interactive mode to exit

OSC Toggle Management:
  osc toggle list        List all configured OSC toggle triggers
  osc toggle test        Test sending an OSC parameter value
  osc toggle create      Create a new OSC toggle trigger for a redeem
  osc toggle update      Update an existing trigger configuration
  osc toggle delete      Delete a trigger
  osc toggle active      Show currently active toggles

OSC Destinations:
  osc set vrcdest        Set VRChat OSC destination (default: 127.0.0.1:9000)
  osc set robodest       Set Robot OSC destination

Examples:
  osc start                                    # Start the OSC service
  osc chatbox Hello world!                     # Send message to VRChat
  osc toggle test /avatar/parameters/Wings bool true 30  # Test wings for 30 seconds
  osc toggle create <redeem_id> /avatar/parameters/Ears bool true false 60
  osc toggle list                              # See all configured triggers
  osc set vrcdest 192.168.1.100:9000          # Change VRChat OSC destination

Toggle Types:
  bool   - Boolean values (true/false)
  int    - Integer values (whole numbers)
  float  - Floating point values (decimals)

Notes:
- OSC service must be running to send parameters or chatbox messages
- Toggle durations are in seconds; omit for permanent toggles
- Use 'osc raw' to debug incoming OSC messages from VRChat
- Default VRChat OSC port is 9000, but can be changed in VRChat settings
"#;