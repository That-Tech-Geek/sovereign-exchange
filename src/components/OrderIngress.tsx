import React, { useState } from 'react';
import { CountryTickerInfo } from '../types';
import { Send, Binary, Play, Zap, Flame, Terminal, Check, Copy, Bot } from 'lucide-react';

interface OrderIngressProps {
  ticker: CountryTickerInfo;
  onInjectOrder: (
    side: 0 | 1,
    priceCents: number,
    qty: number,
    accountId: number
  ) => void;
  onInjectBurst: (count: number, aggressive: boolean) => void;
  nextOrderId: number;
  isSimulating?: boolean;
  onToggleSimulating?: () => void;
  onSpinUpAll10kBots?: () => void;
}

export const OrderIngress: React.FC<OrderIngressProps> = ({
  ticker,
  onInjectOrder,
  onInjectBurst,
  nextOrderId,
  isSimulating = false,
  onToggleSimulating,
  onSpinUpAll10kBots,
}) => {
  const [side, setSide] = useState<0 | 1>(0); // 0 = Buy, 1 = Sell
  const [priceInput, setPriceInput] = useState<string>((ticker.basePrice / 100).toFixed(2));
  const [qtyInput, setQtyInput] = useState<string>('100');
  const [accountId, setAccountId] = useState<number>(404);
  const [copiedCode, setCopiedCode] = useState(false);

  const priceCents = Math.round(parseFloat(priceInput || '0') * 100);
  const qty = parseInt(qtyInput || '0', 10);

  const handleCopyCommand = () => {
    navigator.clipboard.writeText('./scripts/spin_up_bots.sh');
    setCopiedCode(true);
    setTimeout(() => setCopiedCode(false), 2000);
  };

  // Synthesize current 32-byte binary packet representation
  const packetHex = React.useMemo(() => {
    const buffer = new ArrayBuffer(32);
    const view = new DataView(buffer);
    // order_id: u64 (little-endian)
    view.setBigUint64(0, BigInt(nextOrderId), true);
    // account_id: u32
    view.setUint32(8, accountId, true);
    // ticker_id: u16
    view.setUint16(12, ticker.id, true);
    // side: u8
    view.setUint8(14, side);
    // price: u32
    view.setUint32(15, priceCents, true);
    // quantity: u32
    view.setUint32(19, qty, true);
    // timestamp: u64
    view.setBigUint64(23, BigInt(Date.now()) * 1_000_000n, true);
    // _pad: u8
    view.setUint8(31, 0);

    const bytes = new Uint8Array(buffer);
    return Array.from(bytes)
      .map((b) => b.toString(16).padStart(2, '0').toUpperCase())
      .join(' ');
  }, [nextOrderId, accountId, ticker.id, side, priceCents, qty]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (priceCents <= 0 || qty <= 0) return;
    onInjectOrder(side, priceCents, qty, accountId);
  };

  return (
    <div className="bg-zinc-900/90 border border-zinc-800 rounded-xl overflow-hidden flex flex-col shadow-lg">
      <div className="p-3.5 border-b border-zinc-800 bg-zinc-950 flex items-center justify-between">
        <div className="flex items-center space-x-2">
          <Send className="w-4 h-4 text-emerald-400" />
          <h2 className="font-bold text-zinc-100 text-sm tracking-tight">ORDER INGRESS (UDP)</h2>
        </div>
        <span className="text-[10px] font-mono bg-zinc-800 px-2 py-0.5 rounded text-zinc-400">
          UDP Port :8888
        </span>
      </div>

      <div className="p-4 space-y-4">
        {/* Manual Order Form */}
        <form onSubmit={handleSubmit} className="space-y-3 font-mono text-xs">
          {/* Side Selector */}
          <div className="grid grid-cols-2 gap-2">
            <button
              type="button"
              onClick={() => setSide(0)}
              className={`py-1.5 rounded text-xs font-bold transition-all ${
                side === 0
                  ? 'bg-emerald-500 text-zinc-950 shadow-md shadow-emerald-950'
                  : 'bg-zinc-800 text-zinc-400 hover:text-zinc-200'
              }`}
            >
              BUY {ticker.symbol}
            </button>
            <button
              type="button"
              onClick={() => setSide(1)}
              className={`py-1.5 rounded text-xs font-bold transition-all ${
                side === 1
                  ? 'bg-rose-500 text-zinc-950 shadow-md shadow-rose-950'
                  : 'bg-zinc-800 text-zinc-400 hover:text-zinc-200'
              }`}
            >
              SELL {ticker.symbol}
            </button>
          </div>

          {/* Price and Quantity */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-[10px] text-zinc-400 mb-1">
                PRICE ({ticker.currency})
              </label>
              <input
                type="number"
                step="0.01"
                value={priceInput}
                onChange={(e) => setPriceInput(e.target.value)}
                className="w-full bg-zinc-950 border border-zinc-800 rounded px-2.5 py-1.5 text-zinc-100 focus:outline-none focus:border-emerald-500 font-mono"
              />
            </div>

            <div>
              <label className="block text-[10px] text-zinc-400 mb-1">QUANTITY</label>
              <input
                type="number"
                step="10"
                value={qtyInput}
                onChange={(e) => setQtyInput(e.target.value)}
                className="w-full bg-zinc-950 border border-zinc-800 rounded px-2.5 py-1.5 text-zinc-100 focus:outline-none focus:border-emerald-500 font-mono"
              />
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-[10px] text-zinc-400 mb-1">BOT / ACCOUNT ID</label>
              <input
                type="number"
                value={accountId}
                onChange={(e) => setAccountId(parseInt(e.target.value, 10) || 100)}
                className="w-full bg-zinc-950 border border-zinc-800 rounded px-2.5 py-1.5 text-zinc-100 focus:outline-none focus:border-emerald-500 font-mono"
              />
            </div>

            <div className="flex items-end">
              <button
                type="submit"
                className={`w-full py-1.5 px-3 rounded font-bold transition-all flex items-center justify-center space-x-1 ${
                  side === 0
                    ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/50 hover:bg-emerald-500/30'
                    : 'bg-rose-500/20 text-rose-300 border border-rose-500/50 hover:bg-rose-500/30'
                }`}
              >
                <Zap className="w-3.5 h-3.5" />
                <span>Transmit Order</span>
              </button>
            </div>
          </div>
        </form>

        {/* 32-Byte Binary Protocol Inspector */}
        <div className="bg-zinc-950 border border-zinc-800/80 rounded-lg p-2.5 space-y-1.5">
          <div className="flex items-center justify-between text-[11px] font-mono text-zinc-400">
            <div className="flex items-center space-x-1.5">
              <Binary className="w-3.5 h-3.5 text-cyan-400" />
              <span className="font-semibold text-zinc-300">Raw OrderPacket (32 Bytes)</span>
            </div>
            <span className="text-[10px] text-zinc-500">repr(C, packed)</span>
          </div>

          {/* Hex display */}
          <div className="font-mono text-[10px] text-zinc-300 bg-zinc-900/80 p-2 rounded border border-zinc-800/50 break-all leading-relaxed select-all">
            {packetHex}
          </div>

          <div className="grid grid-cols-4 gap-1 text-[9px] font-mono text-zinc-500 pt-1">
            <div>O_ID: 8B</div>
            <div>ACC: 4B</div>
            <div>TCK: 2B</div>
            <div>SIDE: 1B</div>
            <div>PRC: 4B</div>
            <div>QTY: 4B</div>
            <div>TS: 8B</div>
            <div>PAD: 1B</div>
          </div>
        </div>

        {/* 10,000 Bot Swarm Master Controller */}
        <div className="pt-3 border-t border-zinc-800/80 space-y-2.5">
          <div className="flex items-center justify-between">
            <span className="text-xs font-mono font-semibold text-zinc-200 flex items-center gap-1.5">
              <Bot className="w-3.5 h-3.5 text-emerald-400" />
              10k Bot Swarm Controller
            </span>
            <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 font-semibold">
              Scaled 100k → 10k
            </span>
          </div>

          {/* Master 1-Click Spin-Up All 10,000 Bots */}
          <button
            id="spin-up-10k-bots-btn"
            onClick={onSpinUpAll10kBots || onToggleSimulating}
            className={`w-full py-2 px-3 rounded-lg text-xs font-mono font-bold tracking-wide transition-all flex items-center justify-center gap-2 shadow-md ${
              isSimulating
                ? 'bg-rose-500/20 text-rose-300 border border-rose-500/40 hover:bg-rose-500/30'
                : 'bg-emerald-500 text-zinc-950 hover:bg-emerald-400 shadow-emerald-950/50'
            }`}
          >
            <Zap className="w-4 h-4 fill-current" />
            <span>{isSimulating ? 'PAUSE 10,000 BOT SWARM' : '⚡ SPIN UP ALL 10,000 BOTS'}</span>
          </button>

          {/* Swarm Archetype Distribution Grid */}
          <div className="grid grid-cols-2 gap-1.5 text-[10px] font-mono bg-zinc-950/80 p-2 rounded-md border border-zinc-800/80">
            <div className="flex items-center justify-between text-zinc-400">
              <span>🏦 Market Makers:</span>
              <span className="text-emerald-400 font-semibold">5,000 (50%)</span>
            </div>
            <div className="flex items-center justify-between text-zinc-400">
              <span>⚡ Momentum Takers:</span>
              <span className="text-amber-400 font-semibold">2,500 (25%)</span>
            </div>
            <div className="flex items-center justify-between text-zinc-400">
              <span>🌐 Sovereign Arbs:</span>
              <span className="text-cyan-400 font-semibold">1,500 (15%)</span>
            </div>
            <div className="flex items-center justify-between text-zinc-400">
              <span>🎲 Noise Traders:</span>
              <span className="text-indigo-400 font-semibold">1,000 (10%)</span>
            </div>
          </div>

          {/* Single Code to Run All Bots (Terminal Runner) */}
          <div className="space-y-1">
            <div className="flex items-center justify-between text-[10px] font-mono text-zinc-400">
              <span className="flex items-center gap-1">
                <Terminal className="w-3 h-3 text-zinc-500" />
                Single Command to Run 10k Bots:
              </span>
              <button
                onClick={handleCopyCommand}
                className="flex items-center gap-1 text-emerald-400 hover:text-emerald-300 font-semibold transition-colors"
                title="Copy single run command"
              >
                {copiedCode ? (
                  <>
                    <Check className="w-3 h-3 text-emerald-400" />
                    <span>Copied!</span>
                  </>
                ) : (
                  <>
                    <Copy className="w-3 h-3" />
                    <span>Copy Command</span>
                  </>
                )}
              </button>
            </div>
            <div className="bg-zinc-950 border border-zinc-800 rounded p-2 text-[11px] font-mono text-zinc-300 flex items-center justify-between select-all overflow-x-auto">
              <code>./scripts/spin_up_bots.sh</code>
              <span className="text-[10px] text-zinc-500 ml-2 whitespace-nowrap">or npm run bots</span>
            </div>
          </div>

          {/* Quick Swarm Waves Ingress */}
          <div className="grid grid-cols-3 gap-1.5 pt-1">
            <button
              onClick={() => onInjectBurst(1000, false)}
              className="py-1.5 px-2 rounded bg-zinc-800 text-zinc-300 hover:bg-zinc-700 text-xs font-mono font-medium transition-colors"
              title="Inject 1,000 passive bot orders"
            >
              +1k Wave
            </button>
            <button
              onClick={() => onInjectBurst(5000, true)}
              className="py-1.5 px-2 rounded bg-amber-500/15 text-amber-300 border border-amber-500/30 hover:bg-amber-500/25 text-xs font-mono font-medium transition-colors"
              title="Inject 5,000 crossing swarm orders"
            >
              +5k Half
            </button>
            <button
              onClick={() => onInjectBurst(10000, true)}
              className="py-1.5 px-2 rounded bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 hover:bg-emerald-500/30 text-xs font-mono font-bold transition-colors"
              title="Inject 10,000 complete bot swarm burst"
            >
              +10k Swarm
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
