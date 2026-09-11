import React from 'react';
import { PerformanceStats } from '../types';
import { RefreshCw, Database, GitBranch, Cpu, Activity, ArrowRight } from 'lucide-react';

interface RingAndScavengerProps {
  stats: PerformanceStats;
}

export const RingAndScavenger: React.FC<RingAndScavengerProps> = ({ stats }) => {
  const ringCapacity = 1_048_576;
  const ringDepthPct = Math.min(100, (stats.ringBufferDepth / 1000) * 100);

  // WAL Chunk Progress toward 1GB
  const targetWalBytes = 1_000_000_000;
  const walPct = Math.min(100, (stats.walBytesBuffered / targetWalBytes) * 100);

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* RingBuffer Card */}
        <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <RefreshCw className="w-4 h-4 text-cyan-400" />
              <h3 className="font-bold font-mono text-zinc-100 text-sm">
                LMAX RING BUFFER (CROSSBEAM MPMC)
              </h3>
            </div>
            <span className="text-xs font-mono text-cyan-400 bg-cyan-950/60 px-2 py-0.5 rounded border border-cyan-800/40">
              Lock-Free Ring
            </span>
          </div>

          <p className="text-xs text-zinc-400 font-mono leading-relaxed">
            Non-blocking communication bridge between UDP network ingress (multiple listener threads) and Core 0 matching engine. Uses power-of-two bounded capacity for bitwise mask wrapping.
          </p>

          <div className="bg-zinc-950 border border-zinc-800 rounded-lg p-4 space-y-3 font-mono text-xs">
            <div className="flex justify-between text-zinc-400 text-[11px]">
              <span>Capacity: {ringCapacity.toLocaleString()} slots (2^20)</span>
              <span>Pending Orders: {stats.ringBufferDepth}</span>
            </div>

            {/* Depth progress meter */}
            <div className="w-full bg-zinc-900 h-3 rounded-full overflow-hidden border border-zinc-800">
              <div
                className="h-full bg-cyan-500 rounded-full transition-all duration-200"
                style={{ width: `${Math.max(2, ringDepthPct)}%` }}
              />
            </div>

            <div className="grid grid-cols-3 gap-2 pt-2 border-t border-zinc-900 text-[11px]">
              <div>
                <span className="text-zinc-500 block">Producer Method</span>
                <span className="text-zinc-200 font-semibold">tx.try_send(idx)</span>
              </div>
              <div>
                <span className="text-zinc-500 block">Consumer Method</span>
                <span className="text-emerald-400 font-semibold">rx.try_recv()</span>
              </div>
              <div>
                <span className="text-zinc-500 block">Hot Path Contention</span>
                <span className="text-emerald-400 font-semibold">Zero Locks</span>
              </div>
            </div>
          </div>
        </div>

        {/* Scavenger Card */}
        <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <Cpu className="w-4 h-4 text-indigo-400" />
              <h3 className="font-bold font-mono text-zinc-100 text-sm">
                CORE 1 SCAVENGER (ASYNC PERSISTENCE)
              </h3>
            </div>
            <span className="text-xs font-mono text-indigo-400 bg-indigo-950/60 px-2 py-0.5 rounded border border-indigo-800/40">
              Pinned to Core 1
            </span>
          </div>

          <p className="text-xs text-zinc-400 font-mono leading-relaxed">
            Consumes executed Trade events asynchronously via an unbounded MPSC channel. Handles all slow I/O (Firestore L2 sync, 60s snapshots, and 1GB WAL compression) without ever stalling Core 0.
          </p>

          <div className="bg-zinc-950 border border-zinc-800 rounded-lg p-4 space-y-3 font-mono text-xs">
            <div className="grid grid-cols-3 gap-3 text-[11px]">
              <div className="bg-zinc-900/80 p-2 rounded border border-zinc-800/60">
                <span className="text-zinc-500 block">Firestore Flushes</span>
                <span className="text-emerald-400 font-bold text-sm">
                  {stats.firestoreFlushes.toLocaleString()}
                </span>
                <span className="text-[10px] text-zinc-500">Every 500ms</span>
              </div>

              <div className="bg-zinc-900/80 p-2 rounded border border-zinc-800/60">
                <span className="text-zinc-500 block">Snapshots</span>
                <span className="text-indigo-300 font-bold text-sm">
                  {stats.snapshotsTaken}
                </span>
                <span className="text-[10px] text-zinc-500">Every 60s</span>
              </div>

              <div className="bg-zinc-900/80 p-2 rounded border border-zinc-800/60">
                <span className="text-zinc-500 block">WAL Buffer</span>
                <span className="text-amber-300 font-bold text-sm">
                  {(stats.walBytesBuffered / 1024).toFixed(1)} KB
                </span>
                <span className="text-[10px] text-zinc-500">Target 1GB commit</span>
              </div>
            </div>

            <div className="pt-2">
              <div className="flex justify-between text-[11px] text-zinc-400 mb-1">
                <span>WAL Chunk Rotation toward GitHub Archival</span>
                <span>{(stats.walBytesBuffered / (1024 * 1024)).toFixed(2)} MB / 1,000 MB</span>
              </div>
              <div className="w-full bg-zinc-900 h-2 rounded-full overflow-hidden border border-zinc-800">
                <div
                  className="h-full bg-amber-500 rounded-full transition-all duration-300"
                  style={{ width: `${Math.max(1, walPct)}%` }}
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Complete Dataflow Diagram */}
      <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm space-y-3 font-mono text-xs">
        <h4 className="text-xs font-bold text-zinc-300 uppercase tracking-wider">
          Complete Pipeline Flow: UDP Socket → Core 0 Matching → Core 1 Scavenger
        </h4>

        <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
          <div className="bg-zinc-950 p-3 rounded-lg border border-zinc-800 space-y-1">
            <span className="text-[10px] text-cyan-400 font-semibold block">STEP 1: INGRESS</span>
            <div className="font-bold text-zinc-200">UDP Packet (32B)</div>
            <div className="text-[11px] text-zinc-500">
              Zero-copy OrderPacket cast, slot allocated in OrderPool, index sent to RingBuffer.
            </div>
          </div>

          <div className="bg-zinc-950 p-3 rounded-lg border border-emerald-900/40 space-y-1">
            <span className="text-[10px] text-emerald-400 font-semibold block">STEP 2: CORE 0 MATCH</span>
            <div className="font-bold text-zinc-200">Hot Path CLOB</div>
            <div className="text-[11px] text-zinc-500">
              BTreeMap check best price, traverse PriceLevel FIFO list, write trades to stack array.
            </div>
          </div>

          <div className="bg-zinc-950 p-3 rounded-lg border border-zinc-800 space-y-1">
            <span className="text-[10px] text-indigo-400 font-semibold block">STEP 3: SLAB RECYCLE</span>
            <div className="font-bold text-zinc-200">O(1) Free List</div>
            <div className="text-[11px] text-zinc-500">
              Filled orders immediately returned to free_head without calling free() or OS.
            </div>
          </div>

          <div className="bg-zinc-950 p-3 rounded-lg border border-amber-900/40 space-y-1">
            <span className="text-[10px] text-amber-400 font-semibold block">STEP 4: SCAVENGER</span>
            <div className="font-bold text-zinc-200">Core 1 Persistence</div>
            <div className="text-[11px] text-zinc-500">
              Trades flushed to Firestore in 500ms batches, binary WAL compressed for Git chunks.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
