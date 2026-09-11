import React from 'react';
import { ShieldCheck, Cpu, HardDrive, Check, Terminal } from 'lucide-react';

export const SpecsProtocol: React.FC = () => {
  return (
    <div className="space-y-6 max-w-5xl mx-auto">
      {/* 32-Byte Packet Protocol Card */}
      <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="font-mono text-sm font-bold text-zinc-100 flex items-center gap-2">
            <Terminal className="w-4 h-4 text-emerald-400" />
            BINARY NETWORK INGRESS PROTOCOL (32 BYTES)
          </h3>
          <span className="text-xs font-mono text-emerald-400 bg-emerald-950/60 px-2 py-0.5 rounded border border-emerald-800/40">
            UDP :8888 • Zero-Copy Pointer Cast
          </span>
        </div>

        <p className="text-xs text-zinc-400 font-mono leading-relaxed">
          The binary protocol is precisely 32 bytes (a power of two), matching modern CPU cache boundaries. No JSON or Protobuf deserialization occurs on the critical matching path:
        </p>

        {/* Visual Memory Strip */}
        <div className="grid grid-cols-8 gap-1 font-mono text-[11px] text-center">
          <div className="bg-zinc-950 border border-zinc-800 p-2 rounded">
            <span className="text-zinc-500 block text-[9px]">BYTES 0..8</span>
            <span className="text-emerald-400 font-bold">order_id</span>
            <span className="text-zinc-400 block text-[9px]">u64 (8B)</span>
          </div>
          <div className="bg-zinc-950 border border-zinc-800 p-2 rounded">
            <span className="text-zinc-500 block text-[9px]">BYTES 8..12</span>
            <span className="text-cyan-400 font-bold">account_id</span>
            <span className="text-zinc-400 block text-[9px]">u32 (4B)</span>
          </div>
          <div className="bg-zinc-950 border border-zinc-800 p-2 rounded">
            <span className="text-zinc-500 block text-[9px]">BYTES 12..14</span>
            <span className="text-amber-400 font-bold">ticker_id</span>
            <span className="text-zinc-400 block text-[9px]">u16 (2B)</span>
          </div>
          <div className="bg-zinc-950 border border-zinc-800 p-2 rounded">
            <span className="text-zinc-500 block text-[9px]">BYTE 14</span>
            <span className="text-rose-400 font-bold">side</span>
            <span className="text-zinc-400 block text-[9px]">u8 (1B)</span>
          </div>
          <div className="bg-zinc-950 border border-zinc-800 p-2 rounded">
            <span className="text-zinc-500 block text-[9px]">BYTES 15..19</span>
            <span className="text-emerald-400 font-bold">price</span>
            <span className="text-zinc-400 block text-[9px]">u32 (4B)</span>
          </div>
          <div className="bg-zinc-950 border border-zinc-800 p-2 rounded">
            <span className="text-zinc-500 block text-[9px]">BYTES 19..23</span>
            <span className="text-cyan-400 font-bold">quantity</span>
            <span className="text-zinc-400 block text-[9px]">u32 (4B)</span>
          </div>
          <div className="bg-zinc-950 border border-zinc-800 p-2 rounded">
            <span className="text-zinc-500 block text-[9px]">BYTES 23..31</span>
            <span className="text-indigo-400 font-bold">timestamp</span>
            <span className="text-zinc-400 block text-[9px]">u64 (8B)</span>
          </div>
          <div className="bg-zinc-950 border border-zinc-800 p-2 rounded">
            <span className="text-zinc-500 block text-[9px]">BYTE 31</span>
            <span className="text-zinc-500 font-bold">_pad</span>
            <span className="text-zinc-500 block text-[9px]">u8 (1B)</span>
          </div>
        </div>
      </div>

      {/* Latency & Queuing Theory Equations */}
      <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm space-y-4">
        <h3 className="font-mono text-sm font-bold text-zinc-100 flex items-center gap-2">
          <Cpu className="w-4 h-4 text-cyan-400" />
          QUEUING THEORY & LATENCY MODEL (M/M/1 / M/G/1)
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 font-mono text-xs">
          <div className="bg-zinc-950 p-4 rounded-lg border border-zinc-800 space-y-2">
            <span className="text-zinc-400 block text-[10px] uppercase">Service Time Bound</span>
            <div className="text-lg font-bold text-emerald-400">E[S] &lt; 5.0 µs</div>
            <p className="text-zinc-500 text-[11px]">
              Expected service time per order. Measured from ring buffer dequeue to trade stack generation.
            </p>
          </div>

          <div className="bg-zinc-950 p-4 rounded-lg border border-zinc-800 space-y-2">
            <span className="text-zinc-400 block text-[10px] uppercase">Max System Capacity</span>
            <div className="text-lg font-bold text-cyan-400">µ = 200,000 /s</div>
            <p className="text-zinc-500 text-[11px]">
              Peak theoretical throughput on a single dedicated pinned x86-64 execution core.
            </p>
          </div>

          <div className="bg-zinc-950 p-4 rounded-lg border border-zinc-800 space-y-2">
            <span className="text-zinc-400 block text-[10px] uppercase">Target Utilization</span>
            <div className="text-lg font-bold text-amber-400">ρ &lt; 0.60</div>
            <p className="text-zinc-500 text-[11px]">
              At 120k orders/sec arrival rate (λ), system utilization ρ = λ / µ remains under 60%, preventing queue buildup.
            </p>
          </div>
        </div>
      </div>

      {/* Implementation Checklist Verification */}
      <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm space-y-4">
        <h3 className="font-mono text-sm font-bold text-zinc-100 flex items-center gap-2">
          <ShieldCheck className="w-4 h-4 text-emerald-400" />
          PRODUCTION AUDIT & VERIFICATION CHECKLIST
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-3 font-mono text-xs">
          {[
            { label: 'Zero Heap Allocation on Hot Path', status: 'VERIFIED', desc: 'Vec::resize only called once in OrderPool::new' },
            { label: 'Cache-Line Alignment', status: 'VERIFIED', desc: '#[repr(C, align(64))] on Order struct' },
            { label: 'Core 0 / Core 1 Affinity Pinning', status: 'VERIFIED', desc: 'core_affinity::set_for_current prevents context switches' },
            { label: 'Lock-Free Disruptor Ring Buffer', status: 'VERIFIED', desc: 'crossbeam bounded 1,048,576 slots' },
            { label: 'Isolated Scavenger Thread', status: 'VERIFIED', desc: 'All Firestore, disk & WAL I/O offloaded to Core 1' },
            { label: 'Strict Price-Time (FIFO) Priority', status: 'VERIFIED', desc: 'Doubly-linked lists per BTreeMap price level' },
          ].map((item, idx) => (
            <div key={idx} className="bg-zinc-950 p-3 rounded-lg border border-zinc-800 flex items-start space-x-2.5">
              <div className="w-4 h-4 rounded-full bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 mt-0.5 shrink-0">
                <Check className="w-3 h-3" />
              </div>
              <div>
                <div className="font-bold text-zinc-200">{item.label}</div>
                <div className="text-[11px] text-zinc-500 mt-0.5">{item.desc}</div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
