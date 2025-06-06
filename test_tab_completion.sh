#!/bin/bash

echo "Testing OSC tab completion..."
echo ""
echo "When the TUI starts, try these tests:"
echo "1. Type 'osc ' and press TAB - should show subcommands"
echo "2. Type 'osc toggle ' and press TAB - should show nested subcommands"
echo "3. Type 'help osc' - should show OSC help"
echo ""
echo "Press Ctrl+C when done testing."
echo ""

cd /home/kittyn/maowbot
./run-tui.sh