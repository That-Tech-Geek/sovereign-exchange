import React from 'react';
import { PerformanceStats } from '../types';
import { Database, HardDrive, CheckCircle2, ShieldCheck, Layers, Cpu } from 'lucide-react';

interface SlabMemoryMonitorProps {
  stats: PerformanceStats;
}

export const SlabMemoryMonitor: React.FC<SlabMemoryMonitorProps> = ({ stats }) => {
  const maxOrders = 5_000_000;
  const allocated = stats.activeAllocatedOrders;
  const memoryMb = ((maxOrders * 64) / (1024 * 1024)).toFixed(1);
  const utilizationPct = ((allocated / maxOrders) * 100).toFixed(4);

  return (
    <div className="space-y-6">
      {/* Top Banner: Slab Architecture Summary */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-4 shadow-sm">
          <div className="flex items-center justify-between text-zinc-400 mb-1">
            <span className="text-xs font-mono">OrderPool Capacity</span>
            <Database className="w-4 h-4 text-emerald-400" />
          </div>
          <div className="text-xl font-bold font-mono text-zinc-100">
            {maxOrders.toLocaleString()} <span className="text-xs text-zinc-500 font-normal">slots</span>
          </div>
          <div className="text-[11px] text-zinc-500 font-mono mt-1">
            Pre-allocated at startup (Vec::with_capacity)
          </div>
        </div>

        <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-4 shadow-sm">
          <div className="flex items-center justify-between text-zinc-400 mb-1">
            <span className="text-xs font-mono">Static Memory Footprint</span>
            <HardDrive className="w-4 h-4 text-cyan-400" />
          </div>
          <div className="text-xl font-bold font-mono text-zinc-100">
            {memoryMb} <span className="text-xs text-zinc-500 font-normal">MB RAM</span>
          </div>
          <div className="text-[11px] text-zinc-500 font-mono mt-1">
            5M × 64 bytes cache-line aligned
          </div>
        </div>

        <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-4 shadow-sm">
          <div className="flex items-center justify-between text-zinc-400 mb-1">
            <span className="text-xs font-mono">Active Resting Orders</span>
            <Layers className="w-4 h-4 text-indigo-400" />
          </div>
          <div className="text-xl font-bold font-mono text-indigo-300">
            {allocated.toLocaleString()} <span className="text-xs text-zinc-500 font-normal">({utilizationPct}%)</span>
          </div>
          <div className="text-[11px] text-zinc-500 font-mono mt-1">
            Free-list head: <span className="text-zinc-300 font-mono">0x{stats.freeHeadIndex.toString(16).padStart(6, '0').toUpperCase()}</span>
          </div>
        </div>

        <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-4 shadow-sm">
          <div className="flex items-center justify-between text-zinc-400 mb-1">
            <span className="text-xs font-mono">Hot Path Memory Alloc</span>
            <ShieldCheck className="w-4 h-4 text-emerald-400" />
          </div>
          <div className="text-xl font-bold font-mono text-emerald-400">
            0 <span className="text-xs text-zinc-500 font-normal">Bytes (Zero Malloc)</span>
          </div>
          <div className="text-[11px] text-zinc-500 font-mono mt-1">
            Stack array trades + pointer mutations
          </div>
        </div>
      </div>

      {/* Memory Grid & Cache-Line Visualization */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Visual Slab Blocks */}
        <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-bold font-mono text-zinc-100 flex items-center gap-2">
              <Cpu className="w-4 h-4 text-emerald-400" />
              SLAB ALLOCATOR & FREE-LIST TOPOLOGY
            </h3>
            <span className="text-xs font-mono text-emerald-400 bg-emerald-950/60 px-2 py-0.5 rounded border border-emerald-800/40">
              O(1) Alloc / Dealloc
            </span>
          </div>

          <p className="text-xs text-zinc-400 font-mono leading-relaxed">
            Every order is allocated from a single contiguous array in virtual memory indexed by <code className="text-emerald-400">u32</code>. Pointers take only 4 bytes instead of 8 bytes on 64-bit platforms, preserving CPU L1/L2 cache locality.
          </p>

          {/* Graphical Slab Memory Segments */}
          <div className="space-y-2 font-mono text-xs">
            <div className="flex justify-between text-[11px] text-zinc-400">
              <span>Memory Range: 0x00000000 — 0x1312D000 (320 MB)</span>
              <span>Slots 0 .. 5,000,000</span>
            </div>

            <div className="w-full h-8 bg-zinc-950 rounded-lg border border-zinc-800 p-1 flex items-center space-x-1 overflow-hidden">
              {/* Segment 0: Sentinel */}
              <div className="w-4 h-full bg-zinc-700 rounded-sm" title="Slot 0: NULL_ORDER Sentinel" />
              {/* Segment 1: Active Orders */}
              <div
                className="h-full bg-emerald-500 rounded-sm transition-all duration-300 min-w-2"
                style={{ width: `${Math.max(2, (allocated / 50000) * 100)}%` }}
                title={`${allocated} active allocated slots`}
              />
              {/* Segment 2: Free list remainder */}
              <div className="flex-1 h-full bg-zinc-800/50 rounded-sm" title="Available slots in free list" />
            </div>

            <div className="flex items-center justify-between text-[10px] text-zinc-500 pt-1">
              <div className="flex items-center gap-1.5">
                <span className="w-2.5 h-2.5 rounded-sm bg-zinc-700 inline-block" />
                <span>Slot 0 (NULL_ORDER Sentinel)</span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="w-2.5 h-2.5 rounded-sm bg-emerald-500 inline-block" />
                <span>Resting in CLOB</span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="w-2.5 h-2.5 rounded-sm bg-zinc-800 inline-block" />
                <span>Available Free List</span>
              </div>
            </div>
          </div>

          {/* Micro-bench specs */}
          <div className="bg-zinc-950 border border-zinc-800 rounded-lg p-3 space-y-2 font-mono text-xs">
            <div className="text-zinc-300 font-semibold text-[11px] uppercase tracking-wider">
              Memory Access Guarantees
            </div>
            <ul className="space-y-1.5 text-zinc-400 text-[11px]">
              <li className="flex items-center gap-2">
                <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                <span><strong>No Pointer Chasing:</strong> Contiguous memory array fits in CPU L3 cache</span>
              </li>
              <li className="flex items-center gap-2">
                <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                <span><strong>Borrow-Checker Safe:</strong> Uses u32 array offsets, eliminating self-referential pointer issues</span>
              </li>
              <li className="flex items-center gap-2">
                <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                <span><strong>False-Sharing Prevention:</strong> #[repr(C, align(64))] aligns each Order to 64-byte cache lines</span>
              </li>
            </ul>
          </div>
        </div>

        {/* 64-Byte Struct Memory Layout */}
        <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-bold font-mono text-zinc-100 flex items-center gap-2">
              <Layers className="w-4 h-4 text-cyan-400" />
              ORDER STRUCT CACHE LINE (64 BYTES)
            </h3>
            <span className="text-xs font-mono text-cyan-400 bg-cyan-950/60 px-2 py-0.5 rounded border border-cyan-800/40">
              repr(C, align(64))
            </span>
          </div>

          <div className="space-y-2 font-mono text-xs">
            <div className="bg-zinc-950 border border-zinc-800 rounded-lg p-3 overflow-x-auto">
              <table className="w-full text-left text-[11px]">
                <thead>
                  <tr className="text-zinc-500 border-b border-zinc-800 pb-1">
                    <th className="py-1">Field</th>
                    <th className="py-1">Rust Type</th>
                    <th className="py-1">Size</th>
                    <th className="py-1">Offset</th>
                    <th className="py-1">Purpose</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-zinc-900 text-zinc-300">
                  <tr>
                    <td className="py-1 font-semibold text-emerald-400">order_id</td>
                    <td>u64</td>
                    <td>8B</td>
                    <td>0..8</td>
                    <td className="text-zinc-400">Unique client order sequence</td>
                  </tr>
                  <tr>
                    <td className="py-1 font-semibold text-emerald-400">account_id</td>
                    <td>u32</td>
                    <td>4B</td>
                    <td>8..12</td>
                    <td className="text-zinc-400">Bot / trader account identifier</td>
                  </tr>
                  <tr>
                    <td className="py-1 font-semibold text-emerald-400">ticker_id</td>
                    <td>u16</td>
                    <td>2B</td>
                    <td>12..14</td>
                    <td className="text-zinc-400">Country ticker index (0..100)</td>
                  </tr>
                  <tr>
                    <td className="py-1 font-semibold text-emerald-400">side</td>
                    <td>u8</td>
                    <td>1B</td>
                    <td>14..15</td>
                    <td className="text-zinc-400">0 = Buy, 1 = Sell</td>
                  </tr>
                  <tr>
                    <td className="py-1 font-semibold text-emerald-400">price</td>
                    <td>u32</td>
                    <td>4B</td>
                    <td>15..19</td>
                    <td className="text-zinc-400">Scaled integer (10050 = $100.50)</td>
                  </tr>
                  <tr>
                    <td className="py-1 font-semibold text-emerald-400">quantity</td>
                    <td>u32</td>
                    <td>4B</td>
                    <td>19..23</td>
                    <td className="text-zinc-400">Original order quantity</td>
                  </tr>
                  <tr>
                    <td className="py-1 font-semibold text-emerald-400">remaining</td>
                    <td>u32</td>
                    <td>4B</td>
                    <td>23..27</td>
                    <td className="text-zinc-400">Remaining quantity to fill</td>
                  </tr>
                  <tr>
                    <td className="py-1 font-semibold text-cyan-400">next</td>
                    <td>u32</td>
                    <td>4B</td>
                    <td>27..31</td>
                    <td className="text-zinc-400">Linked-list next order in price level</td>
                  </tr>
                  <tr>
                    <td className="py-1 font-semibold text-cyan-400">prev</td>
                    <td>u32</td>
                    <td>4B</td>
                    <td>31..35</td>
                    <td className="text-zinc-400">Linked-list prev order in price level</td>
                  </tr>
                  <tr>
                    <td className="py-1 font-semibold text-emerald-400">timestamp</td>
                    <td>u64</td>
                    <td>8B</td>
                    <td>35..43</td>
                    <td className="text-zinc-400">Arrival unix nanoseconds</td>
                  </tr>
                  <tr>
                    <td className="py-1 font-semibold text-zinc-500">_padding</td>
                    <td>[u8; 21]</td>
                    <td>21B</td>
                    <td>43..64</td>
                    <td className="text-zinc-500">Aligned to exact 64-byte boundary</td>
                  </tr>
                </tbody>
              </table>
            </div>

            <div className="text-[11px] text-zinc-400 pt-1">
              <strong>Total Size:</strong> 64 Bytes exactly (1 cache line). Eliminates CPU cache line ping-pong between cores.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
