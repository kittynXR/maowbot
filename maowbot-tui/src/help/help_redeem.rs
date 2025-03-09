// File: maowbot-tui/src/help/help_redeem.rs
//
// Contains help text for the "redeem" TUI subcommands:
//   redeem list
//   redeem enable <redeemname>
//   redeem pause <redeemname>
//   redeem offline <redeemname>
//   redeem setcost <points> <redeemname>
//   redeem setprompt <prompttext> <redeemname>
//   redeem setplugin <pluginname> <redeemname>
//   redeem setcommand <commandname> <redeemname>
//   redeem setcooldown <seconds> <redeemname>
//   redeem setaccount <accountName> <redeemname>
//   redeem remove <accountName> <redeemname>

pub const REDEEM_HELP_TEXT: &str = r#"
Redeem Command Help
===================

Usage:
  redeem <list|enable|pause|offline|setcost|setprompt|setplugin|setcommand|setcooldown|setaccount|remove> ...

Subcommands:

  redeem list
    Lists all known channel-point redeems in the DB for the default platform ("twitch-eventsub").

  redeem enable <redeemName>
    Sets the redeem’s 'is_active' = true.

  redeem pause <redeemName>
    Sets the redeem’s 'is_active' = false (like pausing it).

  redeem offline <redeemName>
    Toggles whether the redeem is active_offline (i.e., available while the stream is offline).

  redeem setcost <points> <redeemName>
    Updates the cost (integer) in the DB for this redeem.

  redeem setprompt <promptText> <redeemName>
    Demonstration only. The 'prompt' field is not currently in the DB example.
    Shown here as an example of adjusting a user-facing prompt.

  redeem setplugin <pluginName> <redeemName>
    Sets redeem.plugin_name in the DB. If you have a plugin that handles certain redeems, link it here.

  redeem setcommand <commandName> <redeemName>
    Sets redeem.command_name in the DB. Possibly ties a command to run when this redeem triggers.

  redeem setcooldown <seconds> <redeemName>
    Demonstration only. The Redeem struct does not currently have a cooldown field.

  redeem setaccount <accountName> <redeemName>
    Placeholder example for multi-account usage. Not fully implemented in the sample code.

  redeem remove <accountName> <redeemName>
    Removes the redeem from the database. The <accountName> parameter is for tracking which account
    is requesting removal; currently not used except for display.

Examples:

  redeem list
  redeem enable "Hydrate"
  redeem pause "Hydrate"
  redeem offline "Cute"
  redeem setcost 100 "Fancy Reward"
  redeem setplugin "my-reward-plugin" "Fancy Reward"
  redeem setcommand "!mycmd" "Fancy Reward"
  redeem setaccount "KittyN" "Fancy Reward"
  redeem remove "KittyN" "Fancy Reward"

Notes:
  - The code examples assume "twitch-eventsub" as the primary platform for channel point redeems.
  - Adjust for your own environment or pass a platform argument as needed.
"#;
