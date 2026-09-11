import React from 'react';
import { TradeRecord } from '../types';
import { Activity, Shield } from 'lucide-react';

interface TradesTapeProps {
  trades: TradeRecord[];
  totalVolumeDollars: number;
}

export const TradesTape: React.FC<TradesTapeProps> = ({ trades, totalVolumeDollars }) => {
  return (
    <div className="bg-zinc-900/90 border border-zinc-800 rounded-xl overflow-hidden flex flex-col h-full shadow-lg">
      <div className="p-3.5 border-b border-zinc-800 bg-zinc-950 flex items-center justify-between">
        <div className="flex items-center space-x-2">
          <Activity className="w-4 h-4 text-amber-400" />
          <h2 className="font-bold text-zinc-100 text-sm tracking-tight">EXECUTED TRADES TAPE</h2>
        </div>
        <div className="text-right font-mono">
          <span className="text-[10px] text-zinc-500 block">NOTIONAL VOLUME</span>
          <span className="text-xs font-bold text-emerald-400">
            ${totalVolumeDollars.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
          </span>
        </div>
      </div>

      <div className="grid grid-cols-5 px-3 py-1.5 bg-zinc-950/60 border-b border-zinc-800/80 text-[11px] font-mono text-zinc-500">
        <div>TIME</div>
        <div>TICKER</div>
        <div className="text-right">PRICE</div>
        <div className="text-right">QTY</div>
        <div className="text-right">BUY / SELL</div>
      </div>

      <div className="flex-1 overflow-y-auto p-2 space-y-1 font-mono text-xs max-h-[360px] scrollbar-thin">
        {trades.length === 0 ? (
          <div className="py-12 text-center text-zinc-600 text-xs">
            Awaiting trade executions...
          </div>
        ) : (
          trades.map((trade) => (
            <div
              key={trade.id}
              className="grid grid-cols-5 py-1 px-2 rounded bg-zinc-950/40 border border-zinc-800/40 hover:bg-zinc-800/50 transition-colors items-center text-[11px]"
            >
              <div className="text-zinc-400 text-[10px] truncate">
                {trade.timestampFormatted}
              </div>
              <div className="font-bold text-zinc-200">
                {trade.tickerSymbol}
              </div>
              <div className="text-right font-semibold text-amber-300">
                ${(trade.price / 100).toFixed(2)}
              </div>
              <div className="text-right text-zinc-300">
                {trade.qty.toLocaleString()}
              </div>
              <div className="text-right text-zinc-400 text-[10px]">
                <span className="text-emerald-400 font-medium">#{trade.buyer}</span> /{' '}
                <span className="text-rose-400 font-medium">#{trade.seller}</span>
              </div>
            </div>
          ))
        )}
      </div>

      <div className="px-3 py-2 border-t border-zinc-800 bg-zinc-950 flex items-center justify-between text-[10px] text-zinc-500 font-mono">
        <div>Pre-allocated Array [Trade; 64]</div>
        <div>Total: {trades.length} matches logged</div>
      </div>
    </div>
  );
};
