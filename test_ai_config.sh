#!/bin/bash
# Test AI configuration with database storage

echo "AI Configuration Test Script"
echo "============================"
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if server is running
echo -e "${YELLOW}Checking if server is running...${NC}"
if ! pgrep -f "maowbot-server" > /dev/null; then
    echo -e "${RED}Server is not running. Please start the server first.${NC}"
    echo "Run: cd maowbot-server && cargo run"
    exit 1
fi

echo -e "${GREEN}✓ Server is running${NC}"
echo ""

# Test TUI commands
echo -e "${YELLOW}Testing AI provider configuration...${NC}"
echo ""

# Show current status
echo "1. Current AI status:"
./run-tui.sh --command "ai status"
echo ""

# List providers
echo "2. Available providers:"
./run-tui.sh --command "ai provider list"
echo ""

# Configure OpenAI (you'll need to replace with a real key)
echo "3. Configuring OpenAI provider (using test key):"
./run-tui.sh --command "ai provider configure openai --api-key test-key-123 --model gpt-4o"
echo ""

# Test the provider
echo "4. Testing configured provider:"
./run-tui.sh --command "ai provider test openai hello"
echo ""

# Try a chat
echo "5. Testing chat functionality:"
./run-tui.sh --command "ai chat 'Say hello in one word'"
echo ""

echo -e "${YELLOW}Test complete!${NC}"
echo ""
echo "Notes:"
echo "- Replace 'test-key-123' with a real API key for actual testing"
echo "- API keys are stored encrypted in the database"
echo "- Keys persist across server restarts"
echo "- Environment variable OPENAI_API_KEY is used as fallback if no DB config exists"