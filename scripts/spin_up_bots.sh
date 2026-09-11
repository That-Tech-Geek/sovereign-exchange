#!/usr/bin/env bash
# ==============================================================================
# EXCHANGE CORE - 10K BOT SWARM LAUNCHER
#
# Single command to spin up all 10,000 algorithmic trading bots
# (Scaled down from 100k to 10k bots for peak memory locality & sub-5µs latency)
#
# Usage:
#   ./scripts/spin_up_bots.sh
#   ./scripts/spin_up_bots.sh --bots 10000
# ==============================================================================

set -e

BOT_COUNT=${1:-10000}
if [ "$1" == "--bots" ] && [ -n "$2" ]; then
  BOT_COUNT=$2
fi

echo "============================================================"
echo "🤖 LAUNCHING 10K BOT SWARM (Exchange Core Engine)"
echo "   Target Swarm Size: $BOT_COUNT bots (Accounts #1,000 to #$((1000 + BOT_COUNT)))"
echo "   Zero Heap Allocation | Pinned LMAX Ring Buffer"
echo "============================================================"

# Check for Cargo / Rust and C compiler linker
if (command -v cargo &> /dev/null || [ -f "$HOME/.cargo/bin/cargo" ]) && command -v cc &> /dev/null; then
  CARGO_BIN=$(command -v cargo || echo "$HOME/.cargo/bin/cargo")
  echo "⚡ Executing native Rust bot swarm via $CARGO_BIN..."
  cd exchange_core
  $CARGO_BIN run --release --bin bot_swarm -- --bots "$BOT_COUNT"
  exit 0
fi

# High-performance Node/TSX bot swarm runner
if command -v npx &> /dev/null || command -v node &> /dev/null; then
  echo "⚡ Executing high-performance Node/TSX bot swarm runner..."
  npx tsx scripts/bot_swarm.ts --bots "$BOT_COUNT"
  exit 0
fi

echo "❌ Neither Rust nor Node.js runtime found to run bot swarm."
exit 1
