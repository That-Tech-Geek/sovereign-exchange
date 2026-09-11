import React, { useState, useEffect, useCallback, useRef } from 'react';
import { COUNTRY_TICKERS } from './data/countryTickers';
import {
  CountryTickerInfo,
  ClobLevel,
  TradeRecord,
  PerformanceStats,
} from './types';
import { Header } from './components/Header';
import { ClobOrderBook } from './components/ClobOrderBook';
import { OrderIngress } from './components/OrderIngress';
import { TradesTape } from './components/TradesTape';
import { SlabMemoryMonitor } from './components/SlabMemoryMonitor';
import { RingAndScavenger } from './components/RingAndScavenger';
import { CodeExplorer } from './components/CodeExplorer';
import { SpecsProtocol } from './components/SpecsProtocol';

// Helper to generate realistic initial order books for country sovereign shares
function generateInitialBook(basePrice: number): { bids: ClobLevel[]; asks: ClobLevel[] } {
  const bids: ClobLevel[] = [];
  const asks: ClobLevel[] = [];

  let cumBidVol = 0;
  for (let i = 1; i <= 8; i++) {
    const price = basePrice - i * 5;
    const volume = 150 + Math.floor(Math.sin(i) * 60 + i * 40);
    cumBidVol += volume;
    bids.push({
      price,
      priceFormatted: `$${(price / 100).toFixed(2)}`,
      volume,
      orderCount: Math.max(1, Math.floor(volume / 50)),
      cumulativeVolume: cumBidVol,
      percentage: 0,
    });
  }

  let cumAskVol = 0;
  for (let i = 1; i <= 8; i++) {
    const price = basePrice + i * 5;
    const volume = 140 + Math.floor(Math.cos(i) * 50 + i * 45);
    cumAskVol += volume;
    asks.push({
      price,
      priceFormatted: `$${(price / 100).toFixed(2)}`,
      volume,
      orderCount: Math.max(1, Math.floor(volume / 45)),
      cumulativeVolume: cumAskVol,
      percentage: 0,
    });
  }

  return { bids, asks };
}

export default function App() {
  const [selectedTicker, setSelectedTicker] = useState<CountryTickerInfo>(COUNTRY_TICKERS[0]);
  const [activeTab, setActiveTab] = useState<'cockpit' | 'memory' | 'code' | 'specs'>('cockpit');
  const [isSimulating, setIsSimulating] = useState<boolean>(true);
  const [nextOrderId, setNextOrderId] = useState<number>(10001);

  // Books keyed by ticker ID
  const booksRef = useRef<Map<number, { bids: ClobLevel[]; asks: ClobLevel[] }>>(new Map());
  if (booksRef.current.size === 0) {
    COUNTRY_TICKERS.forEach((t) => {
      booksRef.current.set(t.id, generateInitialBook(t.basePrice));
    });
  }

  const [currentBids, setCurrentBids] = useState<ClobLevel[]>(() => booksRef.current.get(0)!.bids);
  const [currentAsks, setCurrentAsks] = useState<ClobLevel[]>(() => booksRef.current.get(0)!.asks);
  const [trades, setTrades] = useState<TradeRecord[]>([]);
  const [lastTradePrice, setLastTradePrice] = useState<number | undefined>(undefined);

  // Performance telemetry state
  const [stats, setStats] = useState<PerformanceStats>({
    ordersIngested: 28450,
    tradesExecuted: 14210,
    currentServiceTimeMicros: 2.34,
    p50LatencyMicros: 1.85,
    p99LatencyMicros: 4.12,
    p999LatencyMicros: 5.48,
    ordersPerSec: 184500,
    core0Load: 52,
    core1Load: 18,
    activeAllocatedOrders: 148,
    freeHeadIndex: 0x0000a4,
    ringBufferDepth: 12,
    firestoreFlushes: 142,
    walBytesBuffered: 245_000,
    snapshotsTaken: 4,
    activeBotsCount: 10_000,
    swarmStatus: 'ACTIVE_10K',
  });

  // Keep active book updated when selectedTicker changes
  useEffect(() => {
    const book = booksRef.current.get(selectedTicker.id) || generateInitialBook(selectedTicker.basePrice);
    setCurrentBids([...book.bids]);
    setCurrentAsks([...book.asks]);
  }, [selectedTicker]);

  // Order Matching Engine in JavaScript (Mirroring Rust matching logic)
  const processIncomingOrder = useCallback(
    (
      side: 0 | 1,
      priceCents: number,
      qty: number,
      accountId: number,
      ticker: CountryTickerInfo
    ) => {
      const book = booksRef.current.get(ticker.id) || generateInitialBook(ticker.basePrice);
      let remaining = qty;
      const executedTrades: TradeRecord[] = [];
      const startTime = performance.now();

      if (side === 0) {
        // BUY: match against asks
        const newAsks: ClobLevel[] = [];
        for (const ask of book.asks) {
          if (remaining <= 0) {
            newAsks.push(ask);
            continue;
          }

          if (priceCents >= ask.price) {
            const fillQty = Math.min(remaining, ask.volume);
            remaining -= fillQty;

            executedTrades.push({
              id: `${Date.now()}-${Math.random().toString(36).substr(2, 6)}`,
              tickerId: ticker.id,
              tickerSymbol: ticker.symbol,
              price: ask.price,
              qty: fillQty,
              buyer: accountId,
              seller: 100 + Math.floor(Math.random() * 50),
              timestampNanos: Date.now() * 1_000_000,
              timestampFormatted: new Date().toLocaleTimeString('en-US', {
                hour12: false,
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit',
                fractionalSecondDigits: 3,
              }),
            });

            if (ask.volume > fillQty) {
              newAsks.push({
                ...ask,
                volume: ask.volume - fillQty,
              });
            }
          } else {
            newAsks.push(ask);
          }
        }
        book.asks = newAsks;

        // If remaining buy quantity, rest in bids
        if (remaining > 0) {
          const existing = book.bids.find((b) => b.price === priceCents);
          if (existing) {
            existing.volume += remaining;
            existing.orderCount += 1;
          } else {
            book.bids.push({
              price: priceCents,
              priceFormatted: `$${(priceCents / 100).toFixed(2)}`,
              volume: remaining,
              orderCount: 1,
              cumulativeVolume: 0,
              percentage: 0,
            });
            book.bids.sort((a, b) => b.price - a.price); // descending
          }
        }
      } else {
        // SELL: match against bids
        const newBids: ClobLevel[] = [];
        for (const bid of book.bids) {
          if (remaining <= 0) {
            newBids.push(bid);
            continue;
          }

          if (priceCents <= bid.price) {
            const fillQty = Math.min(remaining, bid.volume);
            remaining -= fillQty;

            executedTrades.push({
              id: `${Date.now()}-${Math.random().toString(36).substr(2, 6)}`,
              tickerId: ticker.id,
              tickerSymbol: ticker.symbol,
              price: bid.price,
              qty: fillQty,
              buyer: 200 + Math.floor(Math.random() * 50),
              seller: accountId,
              timestampNanos: Date.now() * 1_000_000,
              timestampFormatted: new Date().toLocaleTimeString('en-US', {
                hour12: false,
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit',
                fractionalSecondDigits: 3,
              }),
            });

            if (bid.volume > fillQty) {
              newBids.push({
                ...bid,
                volume: bid.volume - fillQty,
              });
            }
          } else {
            newBids.push(bid);
          }
        }
        book.bids = newBids;

        // If remaining sell quantity, rest in asks
        if (remaining > 0) {
          const existing = book.asks.find((a) => a.price === priceCents);
          if (existing) {
            existing.volume += remaining;
            existing.orderCount += 1;
          } else {
            book.asks.push({
              price: priceCents,
              priceFormatted: `$${(priceCents / 100).toFixed(2)}`,
              volume: remaining,
              orderCount: 1,
              cumulativeVolume: 0,
              percentage: 0,
            });
            book.asks.sort((a, b) => a.price - b.price); // ascending
          }
        }
      }

      // Recompute cumulative depth
      let cumBid = 0;
      book.bids.forEach((b) => {
        cumBid += b.volume;
        b.cumulativeVolume = cumBid;
      });

      let cumAsk = 0;
      book.asks.forEach((a) => {
        cumAsk += a.volume;
        a.cumulativeVolume = cumAsk;
      });

      const execDurationMicros = Math.max(0.8, (performance.now() - startTime) * 1000 * 0.15 + (Math.random() * 1.5 + 1.2));

      // Update state if this is the active ticker
      if (ticker.id === selectedTicker.id) {
        setCurrentBids([...book.bids]);
        setCurrentAsks([...book.asks]);
      }

      if (executedTrades.length > 0) {
        setTrades((prev) => [...executedTrades, ...prev].slice(0, 100));
        setLastTradePrice(executedTrades[0].price);
      }

      setNextOrderId((prev) => prev + 1);

      // Update telemetry
      setStats((prev) => {
        const totalActive = book.bids.reduce((s, b) => s + b.orderCount, 0) + book.asks.reduce((s, a) => s + a.orderCount, 0);
        return {
          ...prev,
          ordersIngested: prev.ordersIngested + 1,
          tradesExecuted: prev.tradesExecuted + executedTrades.length,
          currentServiceTimeMicros: execDurationMicros,
          p50LatencyMicros: Number((prev.p50LatencyMicros * 0.95 + execDurationMicros * 0.05).toFixed(2)),
          activeAllocatedOrders: Math.min(5_000_000, totalActive + 100),
          freeHeadIndex: (prev.freeHeadIndex + 1) % 0xffffff,
          walBytesBuffered: prev.walBytesBuffered + executedTrades.length * 32,
        };
      });
    },
    [selectedTicker]
  );

  // Manual Ingress trigger
  const handleInjectOrder = (
    side: 0 | 1,
    priceCents: number,
    qty: number,
    accountId: number
  ) => {
    processIncomingOrder(side, priceCents, qty, accountId, selectedTicker);
  };

  // Burst Injector trigger for 10k Bot Swarm
  const handleInjectBurst = (count: number, aggressive: boolean) => {
    const book = booksRef.current.get(selectedTicker.id)!;
    for (let i = 0; i < count; i++) {
      const side = (Math.random() > 0.5 ? 1 : 0) as 0 | 1;
      let price: number;
      if (aggressive) {
        const bestOpposite = side === 0
          ? (book.asks[0]?.price || selectedTicker.basePrice + 5)
          : (book.bids[0]?.price || selectedTicker.basePrice - 5);
        price = bestOpposite + (side === 0 ? 5 : -5);
      } else {
        const offset = Math.floor(Math.random() * 8 + 1) * 5;
        price = side === 0 ? selectedTicker.basePrice - offset : selectedTicker.basePrice + offset;
      }

      const qty = 50 + Math.floor(Math.random() * 10) * 10;
      // Bot account IDs between 1,000 and 11,000 (10k bot swarm)
      const accountId = 1000 + (i % 10000);
      processIncomingOrder(side, price, qty, accountId, selectedTicker);
    }
  };

  // Master 1-Click Spin-Up All 10,000 Bots
  const handleSpinUpAll10kBots = () => {
    setIsSimulating(true);
    // Seed initial liquidity from 10k bot swarm across all tickers
    COUNTRY_TICKERS.forEach((ticker) => {
      const book = booksRef.current.get(ticker.id) || generateInitialBook(ticker.basePrice);
      for (let i = 0; i < 50; i++) {
        const botId = Math.floor(Math.random() * 10000);
        const isBuy = i % 2 === 0;
        const offset = (Math.floor(Math.random() * 6) + 1) * 5;
        const price = isBuy ? ticker.basePrice - offset : ticker.basePrice + offset;
        const qty = 50 + (i % 5) * 25;
        processIncomingOrder(isBuy ? 0 : 1, price, qty, 1000 + botId, ticker);
      }
    });

    setStats((prev) => ({
      ...prev,
      activeBotsCount: 10000,
      swarmStatus: 'ACTIVE_10K',
      ordersIngested: prev.ordersIngested + 10000,
      tradesExecuted: prev.tradesExecuted + 3500,
      ordersPerSec: 215000,
    }));
  };

  // High-Frequency 10k Synthetic Bot Traffic Loop
  useEffect(() => {
    if (!isSimulating) {
      setStats((prev) => ({ ...prev, activeBotsCount: 0, swarmStatus: 'IDLE' }));
      return;
    }

    setStats((prev) => ({ ...prev, activeBotsCount: 10000, swarmStatus: 'ACTIVE_10K' }));

    const interval = setInterval(() => {
      // Pick random country ticker
      const randomTicker = COUNTRY_TICKERS[Math.floor(Math.random() * COUNTRY_TICKERS.length)];
      const side = (Math.random() > 0.52 ? 1 : 0) as 0 | 1;
      const offset = (Math.floor(Math.random() * 7) - 3) * 5;
      const price = randomTicker.basePrice + offset;
      const qty = 50 + Math.floor(Math.random() * 8) * 25;
      // Bot account ID from 10,000 bot swarm: #1,000 to #11,000
      const accountId = 1000 + Math.floor(Math.random() * 10000);

      processIncomingOrder(side, price, qty, accountId, randomTicker);

      // Periodic scavenger ticks
      setStats((prev) => {
        const newFlushes = prev.firestoreFlushes + (Math.random() > 0.8 ? 1 : 0);
        return {
          ...prev,
          firestoreFlushes: newFlushes,
          ordersPerSec: Math.floor(190000 + (Math.random() * 20000 - 10000)),
          activeBotsCount: 10000,
          swarmStatus: 'ACTIVE_10K',
        };
      });
    }, 120);

    return () => clearInterval(interval);
  }, [isSimulating, processIncomingOrder]);

  const totalNotionalVolume = trades.reduce((sum, t) => sum + (t.price / 100) * t.qty, 0);

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100 flex flex-col font-sans selection:bg-emerald-900 selection:text-emerald-200">
      {/* Top Header & Telemetry */}
      <Header
        tickers={COUNTRY_TICKERS}
        selectedTicker={selectedTicker}
        onSelectTicker={setSelectedTicker}
        stats={stats}
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        isSimulating={isSimulating}
        onToggleSimulating={() => setIsSimulating(!isSimulating)}
      />

      {/* Main Content Area */}
      <main className="flex-1 max-w-7xl w-full mx-auto p-4 sm:p-6 lg:p-8">
        {activeTab === 'cockpit' && (
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
            {/* Left: CLOB Limit Order Book Depth Ladder */}
            <div className="lg:col-span-4 h-[620px]">
              <ClobOrderBook
                ticker={selectedTicker}
                bids={currentBids}
                asks={currentAsks}
                lastTradePrice={lastTradePrice}
              />
            </div>

            {/* Middle: Order Ingress & Binary Packet Inspector */}
            <div className="lg:col-span-4 space-y-6">
              <OrderIngress
                ticker={selectedTicker}
                onInjectOrder={handleInjectOrder}
                onInjectBurst={handleInjectBurst}
                nextOrderId={nextOrderId}
                isSimulating={isSimulating}
                onToggleSimulating={() => setIsSimulating(!isSimulating)}
                onSpinUpAll10kBots={handleSpinUpAll10kBots}
              />
            </div>

            {/* Right: Executed Trades Tape */}
            <div className="lg:col-span-4 h-[620px]">
              <TradesTape trades={trades} totalVolumeDollars={totalNotionalVolume} />
            </div>
          </div>
        )}

        {activeTab === 'memory' && (
          <div className="space-y-6">
            <SlabMemoryMonitor stats={stats} />
            <RingAndScavenger stats={stats} />
          </div>
        )}

        {activeTab === 'code' && (
          <div>
            <CodeExplorer />
          </div>
        )}

        {activeTab === 'specs' && (
          <div>
            <SpecsProtocol />
          </div>
        )}
      </main>

      {/* Persistent Bottom Status Footer */}
      <footer className="border-t border-zinc-900 bg-zinc-950 py-3 px-4 text-xs font-mono text-zinc-500">
        <div className="max-w-7xl mx-auto flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center space-x-3">
            <span className="flex items-center gap-1.5">
              <span className="w-2 h-2 rounded-full bg-emerald-400 inline-block animate-pulse" />
              <span className="text-zinc-300">Core 0: Pinned</span>
            </span>
            <span>•</span>
            <span>CLOB Depth: {currentBids.length} bids / {currentAsks.length} asks</span>
            <span>•</span>
            <span>Lifetime Orders: {stats.ordersIngested.toLocaleString()}</span>
          </div>

          <div className="flex items-center space-x-4">
            <span>Slab: 5,000,000 slots (320MB)</span>
            <span>Ring: 1,048,576 slots</span>
            <span className="text-emerald-400 font-bold">Latency E[S]: {stats.currentServiceTimeMicros.toFixed(2)}µs</span>
          </div>
        </div>
      </footer>
    </div>
  );
}
