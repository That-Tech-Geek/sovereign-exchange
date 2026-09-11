import React from 'react';
import { ClobLevel, CountryTickerInfo } from '../types';
import { TrendingUp, TrendingDown, ArrowRightLeft } from 'lucide-react';

interface ClobOrderBookProps {
  ticker: CountryTickerInfo;
  bids: ClobLevel[];
  asks: ClobLevel[];
  lastTradePrice?: number;
}

export const ClobOrderBook: React.FC<ClobOrderBookProps> = ({
  ticker,
  bids,
  asks,
  lastTradePrice,
}) => {
  const bestBid = bids.length > 0 ? bids[0].price : null;
  const bestAsk = asks.length > 0 ? asks[0].price : null;
  
  const spread = bestBid && bestAsk ? Math.max(0, bestAsk - bestBid) : 0;
  const spreadDollars = (spread / 100).toFixed(2);
  const midPrice = bestBid && bestAsk ? ((bestBid + bestAsk) / 200).toFixed(2) : ((ticker.basePrice) / 100).toFixed(2);

  // Maximum cumulative volume for visual progress bar scale
  const maxBidVol = bids.length > 0 ? bids[bids.length - 1].cumulativeVolume : 1;
  const maxAskVol = asks.length > 0 ? asks[asks.length - 1].cumulativeVolume : 1;
  const maxScale = Math.max(maxBidVol, maxAskVol, 1);

  // Display top 8 asks (reversed so highest ask is top, lowest ask nearest to spread)
  const displayAsks = [...asks.slice(0, 8)].reverse();
  const displayBids = bids.slice(0, 8);

  return (
    <div className="bg-zinc-900/90 border border-zinc-800 rounded-xl overflow-hidden flex flex-col h-full shadow-lg">
      {/* CLOB Header */}
      <div className="p-3.5 border-b border-zinc-800 bg-zinc-950 flex items-center justify-between">
        <div className="flex items-center space-x-2.5">
          <span className="text-xl">{ticker.flag}</span>
          <div>
            <div className="flex items-center space-x-1.5">
              <span className="font-bold text-zinc-100 tracking-tight">{ticker.symbol}</span>
              <span className="text-xs text-zinc-400 font-normal">({ticker.name})</span>
            </div>
            <span className="text-[10px] font-mono text-zinc-500">
              Ticker #{ticker.id} • Sovereign Shares • {ticker.currency}
            </span>
          </div>
        </div>

        <div className="text-right font-mono">
          <div className="text-xs text-zinc-400">Mid Price</div>
          <div className="text-sm font-bold text-zinc-100">${midPrice}</div>
        </div>
      </div>

      {/* Column Headers */}
      <div className="grid grid-cols-4 px-3 py-1.5 bg-zinc-950/60 border-b border-zinc-800/80 text-[11px] font-mono text-zinc-500">
        <div>PRICE ({ticker.currency})</div>
        <div className="text-right">QTY</div>
        <div className="text-right">ORDERS</div>
        <div className="text-right">TOTAL</div>
      </div>

      {/* Book Body */}
      <div className="flex-1 flex flex-col justify-between overflow-hidden p-2 text-xs font-mono">
        {/* Asks (Sells) */}
        <div className="flex flex-col justify-end space-y-0.5">
          {displayAsks.length === 0 ? (
            <div className="py-6 text-center text-zinc-600 text-[11px]">No asks resting</div>
          ) : (
            displayAsks.map((ask) => {
              const depthPct = Math.min(100, Math.round((ask.cumulativeVolume / maxScale) * 100));
              return (
                <div
                  key={`ask-${ask.price}`}
                  className="relative grid grid-cols-4 py-0.5 px-2 rounded group hover:bg-zinc-800/40 transition-colors"
                >
                  {/* Depth Fill Bar */}
                  <div
                    className="absolute right-0 top-0 bottom-0 bg-rose-500/10 rounded pointer-events-none transition-all duration-150"
                    style={{ width: `${depthPct}%` }}
                  />
                  <div className="font-semibold text-rose-400 z-10">{ask.priceFormatted}</div>
                  <div className="text-right text-zinc-200 z-10">{ask.volume.toLocaleString()}</div>
                  <div className="text-right text-zinc-500 z-10">{ask.orderCount}</div>
                  <div className="text-right text-zinc-400 z-10">{ask.cumulativeVolume.toLocaleString()}</div>
                </div>
              );
            })
          )}
        </div>

        {/* Spread Mid-Bar */}
        <div className="py-2 my-1 px-3 bg-zinc-950 border-y border-zinc-800/90 rounded flex items-center justify-between text-xs font-mono">
          <div className="flex items-center space-x-2">
            <ArrowRightLeft className="w-3.5 h-3.5 text-zinc-500" />
            <span className="text-zinc-400">Spread:</span>
            <span className="text-zinc-200 font-bold">${spreadDollars}</span>
            <span className="text-[10px] text-zinc-500">
              ({bestBid && bestAsk ? ((spread / bestAsk) * 10000).toFixed(1) : 0} bps)
            </span>
          </div>

          {lastTradePrice && (
            <div className="flex items-center space-x-1.5">
              <span className="text-[10px] text-zinc-500 uppercase">Last Match:</span>
              <span className="font-bold text-amber-300">
                ${(lastTradePrice / 100).toFixed(2)}
              </span>
            </div>
          )}
        </div>

        {/* Bids (Buys) */}
        <div className="flex flex-col space-y-0.5">
          {displayBids.length === 0 ? (
            <div className="py-6 text-center text-zinc-600 text-[11px]">No bids resting</div>
          ) : (
            displayBids.map((bid) => {
              const depthPct = Math.min(100, Math.round((bid.cumulativeVolume / maxScale) * 100));
              return (
                <div
                  key={`bid-${bid.price}`}
                  className="relative grid grid-cols-4 py-0.5 px-2 rounded group hover:bg-zinc-800/40 transition-colors"
                >
                  {/* Depth Fill Bar */}
                  <div
                    className="absolute right-0 top-0 bottom-0 bg-emerald-500/10 rounded pointer-events-none transition-all duration-150"
                    style={{ width: `${depthPct}%` }}
                  />
                  <div className="font-semibold text-emerald-400 z-10">{bid.priceFormatted}</div>
                  <div className="text-right text-zinc-200 z-10">{bid.volume.toLocaleString()}</div>
                  <div className="text-right text-zinc-500 z-10">{bid.orderCount}</div>
                  <div className="text-right text-zinc-400 z-10">{bid.cumulativeVolume.toLocaleString()}</div>
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* CLOB Footer */}
      <div className="px-3.5 py-2 border-t border-zinc-800 bg-zinc-950 flex items-center justify-between text-[11px] text-zinc-500 font-mono">
        <div>BTreeMap Key: u32 Scaled Cents</div>
        <div className="flex items-center space-x-2">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 inline-block" />
          <span>FIFO Queue Per Level</span>
        </div>
      </div>
    </div>
  );
};
