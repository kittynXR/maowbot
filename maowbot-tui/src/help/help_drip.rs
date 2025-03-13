// File: maowbot-tui/src/help/help_drip.rs
//
// Detailed help text for the "drip" command group.
//
// This covers the same usage that is implemented in drip.rs, but in a
// “static doc” style for the TUI help system.

pub const DRIP_HELP_TEXT: &str = r#"Drip Command:

Usage:

  drip
    Shows usage help (this text).

Subcommands:

  drip set
    Without arguments => show settable drip parameters (via drip_show_settable).
    Or specify one of:
       drip set i/ignore <prefix>
       drip set s/strip <prefix>
       drip set name <newName>

  drip list
    Lists all locally tracked avatars in the drip database.

  drip fit new <fitName>
    Creates a new “fit” for the current avatar.

  drip fit add <fitName> <param> <value>
    Adds a param override to the named fit.

  drip fit del <fitName> <param> <value>
    Removes a param override from the named fit.

  drip fit w | wear <fitName>
    Applies the named fit’s parameters to the current avatar.

  drip props add <propName> <param> <value>
  drip props del <propName> <param> <value>
  drip props timer <propName> <timerData>
    Manages “props” that can be toggled or timed.

Examples:
  drip set name MyAvatar
  drip list
  drip fit new CasualOutfit
  drip fit add CasualOutfit Clothing Blue
  drip fit wear CasualOutfit
  drip props add fancyHat color Red
"#;
