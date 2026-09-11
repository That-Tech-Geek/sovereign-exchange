import React from 'react';
import { Cpu, Zap, Activity, ShieldCheck, Database, Layers } from 'lucide-react';
import { CountryTickerInfo, PerformanceStats } from '../types';

interface HeaderProps {
  tickers: CountryTickerInfo[];
  selectedTicker: CountryTickerInfo;
  onSelectTicker: (t: CountryTickerInfo) => void;
  stats: PerformanceStats;
  activeTab: 'cockpit' | 'memory' | 'code' | 'specs';
  setActiveTab: (tab: 'cockpit' | 'memory' | 'code' | 'specs') => void;
  isSimulating: boolean;
  onToggleSimulating: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  tickers,
  selectedTicker,
  onSelectTicker,
  stats,
  activeTab,
  setActiveTab,
  isSimulating,
  onToggleSimulating,
}) => {
  return (
    <header className="border-b border-zinc-800 bg-zinc-950/90 backdrop-blur sticky top-0 z-40">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Top telemetry bar */}
        <div className="flex flex-wrap items-center justify-between py-3 border-b border-zinc-800/60 gap-4">
          <div className="flex items-center space-x-3">
            <div className="w-9 h-9 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 shadow-sm shadow-emerald-950">
              <Zap className="w-5 h-5 animate-pulse" />
            </div>
            <div>
              <div className="flex items-center space-x-2">
                <h1 className="text-base font-bold text-zinc-100 tracking-tight">EXCHANGE CORE</h1>
                <span className="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 font-semibold">
                  SUB-5µS ENGINE
                </span>
                <span className="text-[10px] font-mono text-zinc-500">v0.1.0-release</span>
              </div>
              <p className="text-xs text-zinc-400 font-mono">
                Single-Threaded CPU-Pinned CLOB • Zero Heap Alloc • Country Sovereign Shares
              </p>
            </div>
          </div>

          {/* Engine Real-Time Hardware Badges */}
          <div className="flex items-center flex-wrap gap-2 text-xs font-mono">
            {/* Core 0 */}
            <div className="flex items-center space-x-1.5 px-2.5 py-1 rounded-md bg-zinc-900 border border-zinc-800">
              <Cpu className="w-3.5 h-3.5 text-cyan-400" />
              <span className="text-zinc-400">Core 0:</span>
              <span className="text-emerald-400 font-semibold flex items-center gap-1">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-ping inline-block" />
                MATCHING
              </span>
            </div>

            {/* Core 1 */}
            <div className="flex items-center space-x-1.5 px-2.5 py-1 rounded-md bg-zinc-900 border border-zinc-800">
              <Database className="w-3.5 h-3.5 text-indigo-400" />
              <span className="text-zinc-400">Core 1:</span>
              <span className="text-indigo-300 font-medium">SCAVENGER</span>
            </div>

            {/* Latency E[S] */}
            <div className="flex items-center space-x-1.5 px-2.5 py-1 rounded-md bg-emerald-950/40 border border-emerald-800/40">
              <Activity className="w-3.5 h-3.5 text-emerald-400" />
              <span className="text-zinc-400">E[S]:</span>
              <span className="text-emerald-300 font-bold">
                {stats.currentServiceTimeMicros.toFixed(2)} µs
              </span>
              <span className="text-[10px] text-emerald-500/80 font-semibold">(&lt;5µs OK)</span>
            </div>

            {/* Throughput */}
            <div className="flex items-center space-x-1.5 px-2.5 py-1 rounded-md bg-zinc-900 border border-zinc-800">
              <Layers className="w-3.5 h-3.5 text-amber-400" />
              <span className="text-zinc-400">Rate:</span>
              <span className="text-amber-300 font-semibold">
                {stats.ordersPerSec.toLocaleString()} /s
              </span>
            </div>

            {/* 10,000 Bot Swarm Telemetry & Master Toggle */}
            <div className="flex items-center space-x-1.5 px-2.5 py-1 rounded-md bg-zinc-900 border border-zinc-800">
              <span className={`w-2 h-2 rounded-full ${isSimulating ? 'bg-emerald-400 animate-pulse' : 'bg-zinc-600'}`} />
              <span className="text-zinc-400">Bots:</span>
              <span className="text-zinc-200 font-bold">
                {isSimulating ? '10,000' : '0'}{' '}
                <span className="text-[10px] text-zinc-500 font-normal">/ 10k</span>
              </span>
            </div>

            {/* Master Bot Swarm Toggle */}
            <button
              id="master-10k-bot-toggle"
              onClick={onToggleSimulating}
              className={`px-3 py-1 rounded-md text-xs font-semibold tracking-wide transition-all flex items-center space-x-1.5 border shadow-sm ${
                isSimulating
                  ? 'bg-rose-500/20 text-rose-300 border-rose-500/40 hover:bg-rose-500/30 shadow-rose-950/40'
                  : 'bg-emerald-500 text-zinc-950 border-emerald-400 hover:bg-emerald-400 shadow-emerald-950/40'
              }`}
            >
              <span>{isSimulating ? 'Pause 10k Swarm' : 'Spin Up 10,000 Bots'}</span>
            </button>
          </div>
        </div>

        {/* Navigation Tabs & Country Ticker Bar */}
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 py-2">
          {/* Main Navigation Tabs */}
          <nav className="flex space-x-1">
            <button
              onClick={() => setActiveTab('cockpit')}
              className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors ${
                activeTab === 'cockpit'
                  ? 'bg-zinc-800 text-zinc-100 font-semibold shadow-inner'
                  : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-900'
              }`}
            >
              Live Cockpit & CLOB
            </button>
            <button
              onClick={() => setActiveTab('memory')}
              className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors ${
                activeTab === 'memory'
                  ? 'bg-zinc-800 text-zinc-100 font-semibold shadow-inner'
                  : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-900'
              }`}
            >
              Slab Allocator & Rings
            </button>
            <button
              onClick={() => setActiveTab('code')}
              className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors ${
                activeTab === 'code'
                  ? 'bg-zinc-800 text-zinc-100 font-semibold shadow-inner'
                  : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-900'
              }`}
            >
              Rust Codebase (10 Modules)
            </button>
            <button
              onClick={() => setActiveTab('specs')}
              className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors ${
                activeTab === 'specs'
                  ? 'bg-zinc-800 text-zinc-100 font-semibold shadow-inner'
                  : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-900'
              }`}
            >
              32B Protocol & Specs
            </button>
          </nav>

          {/* Country Ticker Pill Selector */}
          <div className="flex items-center space-x-1 overflow-x-auto py-1 scrollbar-none">
            <span className="text-[11px] text-zinc-500 font-mono mr-1">TICKER:</span>
            {tickers.map((ticker) => {
              const isSelected = ticker.id === selectedTicker.id;
              return (
                <button
                  key={ticker.id}
                  onClick={() => onSelectTicker(ticker)}
                  className={`px-2 py-0.5 rounded text-xs font-mono flex items-center space-x-1 transition-all whitespace-nowrap ${
                    isSelected
                      ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/50 font-bold'
                      : 'bg-zinc-900 text-zinc-400 border border-zinc-800/80 hover:text-zinc-200 hover:bg-zinc-800'
                  }`}
                >
                  <span>{ticker.flag}</span>
                  <span>{ticker.symbol}</span>
                </button>
              );
            })}
          </div>
        </div>
      </div>
    </header>
  );
};
