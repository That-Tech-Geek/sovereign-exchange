import React, { useState } from 'react';
import { RUST_CODEBASE } from '../data/rustCodebase';
import { FileCode, Copy, Check, Terminal, ExternalLink } from 'lucide-react';

export const CodeExplorer: React.FC = () => {
  const [selectedFile, setSelectedFile] = useState(RUST_CODEBASE[0]);
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(selectedFile.code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="bg-zinc-900 border border-zinc-800 rounded-xl overflow-hidden shadow-lg flex flex-col md:flex-row min-h-[600px]">
      {/* File Navigation Tree (Sidebar) */}
      <div className="w-full md:w-72 bg-zinc-950 border-r border-zinc-800 flex flex-col p-3 space-y-3 shrink-0">
        <div className="flex items-center space-x-2 text-xs font-mono font-bold text-zinc-300 pb-2 border-b border-zinc-800">
          <Terminal className="w-4 h-4 text-emerald-400" />
          <span>exchange_core/src</span>
        </div>

        <div className="space-y-1 overflow-y-auto flex-1 font-mono text-xs">
          {RUST_CODEBASE.map((file) => {
            const isSelected = file.path === selectedFile.path;
            return (
              <button
                key={file.path}
                onClick={() => setSelectedFile(file)}
                className={`w-full text-left px-2.5 py-2 rounded flex items-center space-x-2 transition-all ${
                  isSelected
                    ? 'bg-emerald-500/15 text-emerald-300 border border-emerald-500/30 font-semibold'
                    : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-900'
                }`}
              >
                <FileCode className={`w-3.5 h-3.5 ${isSelected ? 'text-emerald-400' : 'text-zinc-500'}`} />
                <span className="truncate">{file.name}</span>
              </button>
            );
          })}
        </div>

        {/* Quick CLI tip */}
        <div className="p-2.5 bg-zinc-900 rounded border border-zinc-800/80 text-[10px] font-mono text-zinc-400 space-y-1">
          <span className="text-zinc-300 font-bold block">Build Command:</span>
          <code className="text-emerald-400 block">cargo build --release</code>
          <span className="text-zinc-500 block">LTO enabled, panic=abort</span>
        </div>
      </div>

      {/* Code Viewer Panel */}
      <div className="flex-1 flex flex-col bg-zinc-950/80 overflow-hidden">
        {/* Code Header */}
        <div className="p-3.5 border-b border-zinc-800 bg-zinc-950 flex items-center justify-between">
          <div>
            <div className="font-mono text-xs font-bold text-zinc-200">{selectedFile.path}</div>
            <div className="text-[11px] text-zinc-400 mt-0.5">{selectedFile.description}</div>
          </div>

          <button
            onClick={handleCopy}
            className="flex items-center space-x-1.5 px-3 py-1.5 rounded bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-xs font-mono transition-colors"
          >
            {copied ? (
              <>
                <Check className="w-3.5 h-3.5 text-emerald-400" />
                <span className="text-emerald-400">Copied</span>
              </>
            ) : (
              <>
                <Copy className="w-3.5 h-3.5 text-zinc-400" />
                <span>Copy Code</span>
              </>
            )}
          </button>
        </div>

        {/* Code Block */}
        <div className="flex-1 overflow-auto p-4 bg-zinc-950 text-xs font-mono text-zinc-200 leading-relaxed scrollbar-thin">
          <pre className="selection:bg-emerald-900/60 selection:text-emerald-200">
            <code>{selectedFile.code}</code>
          </pre>
        </div>
      </div>
    </div>
  );
};
