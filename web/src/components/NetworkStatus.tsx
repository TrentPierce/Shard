"use client"

import { useEffect, useState } from "react"
import type { NodeMode } from "@/lib/context"
import type { Topology } from "@/lib/swarm"
import { heartbeatShard } from "@/lib/swarm"
import type { ModelProgress } from "@/lib/webllm"
import { apiUrl } from "@/lib/config"
import { modeLabels } from "./Header"
import { listen } from '@tauri-apps/api/event';

interface NetworkStatusProps {
    mode: NodeMode
    topology: Topology | null
    rustStatus: "connected" | "unreachable" | "downloading"
    webLLMProgress: ModelProgress | null
    webLLMError: string | null
}

interface PeerData {
    peer_id: string
    connected_at: number
    addrs: string[]
}

export default function NetworkStatus({
    mode,
    topology,
    rustStatus,
    webLLMProgress,
    webLLMError,
}: NetworkStatusProps) {
    const [peers, setPeers] = useState<PeerData[]>([])
    const [heartbeat, setHeartbeat] = useState("idle")
    const [pinging, setPinging] = useState(false)
    const [downloadProgress, setDownloadProgress] = useState<number | null>(null)

    useEffect(() => {
        const unlistenProgress = listen<number>('download-progress', (event) => {
            setDownloadProgress(event.payload);
        });

        const unlistenComplete = listen<boolean>('download-complete', () => {
            setDownloadProgress(null);
        });

        return () => {
            unlistenProgress.then(f => f());
            unlistenComplete.then(f => f());
        };
    }, []);

    useEffect(() => {
        const fetchPeers = async () => {
            try {
                const res = await fetch(apiUrl("/v1/system/peers"))
                if (res.ok) {
                    const data = await res.json()
                    setPeers(data.peers ?? [])
                }
            } catch {
                /* sidecar unreachable */
            }
        }

        fetchPeers()
        const interval = setInterval(fetchPeers, 8000)
        return () => clearInterval(interval)
    }, [])

    const doPing = async () => {
        const addr =
            topology?.shard_ws_multiaddr ?? topology?.shard_webrtc_multiaddr
        if (!addr) {
            setHeartbeat("no shard address")
            return
        }
        setPinging(true)
        setHeartbeat("dialing…")
        const result = await heartbeatShard(addr)
        if (result.ok) {
            setHeartbeat(`PONG rtt=${result.rttMs?.toFixed(1)}ms`)
        } else {
            setHeartbeat(`failed: ${result.detail}`)
        }
        setPinging(false)
    }

    return (
        <aside className="hidden lg:flex w-[340px] shrink-0 bg-secondary border-r border-glass-border p-6 xl:p-8 flex-col gap-8 xl:gap-10 min-h-0 overflow-y-auto select-none" role="complementary">
            {/* ── Neural Core ── */}
            <div className="flex flex-col gap-5">
                <div className="flex items-center justify-between">
                    <div className="text-[10px] uppercase tracking-[0.25em] text-muted font-black flex items-center gap-2.5">
                        <div className="w-1.5 h-1.5 rounded-full bg-accent-cyan shadow-glow-cyan"></div>
                        Neural Core
                    </div>
                </div>

                <div className="relative group">
                    <div className="absolute -inset-0.5 bg-gradient-to-br from-accent-cyan to-accent-blue rounded-3xl blur opacity-5 group-hover:opacity-10 transition-smooth"></div>
                    <div className="relative bg-tertiary/30 rounded-2xl p-6 border border-glass-border flex flex-col gap-5">
                        <div className="flex justify-between items-center">
                            <span className="text-[11px] text-secondary font-medium tracking-tight">Sync Protocol</span>
                            <div className="flex flex-col items-end">
                                <span className={`text-[11px] font-black uppercase tracking-wider ${rustStatus === "connected" ? "text-accent-emerald" : rustStatus === "downloading" ? "text-accent-amber" : "text-accent-rose"}`}>
                                    {rustStatus}
                                </span>
                                {rustStatus === "unreachable" && downloadProgress === null && (
                                    <button
                                        className="text-[9px] text-accent-blue hover:text-white underline mt-1 transition-smooth"
                                        onClick={() => window.location.reload()}
                                    >
                                        Re-establish link
                                    </button>
                                )}
                            </div>
                        </div>

                        {downloadProgress !== null && (
                            <div className="flex flex-col gap-2 pt-1">
                                <div className="h-1.5 w-full bg-primary/60 rounded-full overflow-hidden border border-glass-border/30">
                                    <div
                                        className="h-full bg-gradient-to-r from-accent-cyan to-accent-blue shadow-glow-cyan transition-all duration-300"
                                        style={{ width: `${downloadProgress}%` }}
                                    ></div>
                                </div>
                                <div className="flex justify-between text-[10px] items-center">
                                    <span className="text-secondary font-mono">Weight Transfer</span>
                                    <span className="text-primary font-black tabular-nums">{downloadProgress.toFixed(1)}%</span>
                                </div>
                            </div>
                        )}

                        <div className="flex justify-between items-center">
                            <span className="text-[11px] text-secondary font-medium tracking-tight">Active Matrix</span>
                            <span className="text-[11px] font-bold text-white uppercase tracking-widest">{modeLabels[mode]}</span>
                        </div>

                        <div className="grid grid-cols-2 gap-4 pt-4 border-t border-glass-border/40">
                            <div className="flex flex-col gap-1">
                                <span className="text-[9px] uppercase text-muted font-bold tracking-widest leading-none">Control</span>
                                <span className="text-[10px] font-mono text-primary/80">9091 <span className="text-[8px] opacity-30">HEX</span></span>
                            </div>
                            <div className="flex flex-col gap-1">
                                <span className="text-[9px] uppercase text-muted font-bold tracking-widest leading-none">Net Mesh</span>
                                <span className="text-[10px] font-mono text-primary/80">4001 <span className="text-[8px] opacity-30">P2P</span></span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            {/* ── Asset Ledger ── */}
            <div className="flex flex-col gap-5">
                <div className="text-[10px] uppercase tracking-[0.25em] text-muted font-black flex items-center gap-2.5">
                    <div className="w-1.5 h-1.5 rounded-full bg-accent-amber shadow-glow-emerald/20"></div>
                    Ledger
                </div>

                <div className="bg-tertiary/30 rounded-2xl p-6 border border-glass-border flex flex-col gap-5">
                    <div className="flex flex-col gap-1.5">
                        <span className="text-[10px] text-secondary uppercase tracking-widest font-bold">Accumulated Rewards</span>
                        <div className="flex items-baseline gap-2">
                            <div className="text-3xl font-display font-black text-white tracking-tighter">2,450.00</div>
                            <span className="text-xs font-bold text-accent-cyan tracking-widest uppercase">SHRD</span>
                        </div>
                    </div>

                    <div className="grid grid-cols-2 gap-4">
                        <div className="flex flex-col">
                            <span className="text-[9px] text-muted uppercase font-bold tracking-widest">Verifications</span>
                            <span className="text-sm font-black text-white/90">14.2k</span>
                        </div>
                        <div className="flex flex-col">
                            <span className="text-[9px] text-muted uppercase font-bold tracking-widest">Yield Rate</span>
                            <span className="text-sm font-black text-accent-emerald">~12/hr</span>
                        </div>
                    </div>

                    <div className="bg-primary/40 rounded-xl p-3 border border-glass-border flex items-center gap-3">
                        <div className="w-10 h-10 rounded-xl bg-accent-cyan/10 flex items-center justify-center text-accent-cyan shrink-0">
                            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" /></svg>
                        </div>
                        <div className="flex flex-col min-w-0">
                            <span className="text-[9px] uppercase text-muted font-bold tracking-widest">Active Identity</span>
                            <span className="text-[10px] font-mono text-primary/70 truncate uppercase">0x7F4B…3B9A</span>
                        </div>
                    </div>
                </div>
            </div>

            {/* ── Active Swarm ── */}
            <div className="flex flex-col gap-5">
                <div className="flex items-center justify-between">
                    <div className="text-[10px] uppercase tracking-[0.25em] text-muted font-black flex items-center gap-2.5">
                        <div className="w-1.5 h-1.5 rounded-full bg-accent-emerald"></div>
                        Active Swarm
                    </div>
                    <span className="text-[10px] font-bold text-accent-emerald tabular-nums">0{peers.length}</span>
                </div>

                <div className="flex flex-col gap-2.5">
                    {peers.length === 0 ? (
                        <div className="py-8 px-4 border border-dashed border-glass-border rounded-2xl flex flex-col items-center justify-center text-center gap-2">
                            <div className="text-xl opacity-20 filter grayscale">🛰️</div>
                            <div className="text-[11px] text-muted font-medium tracking-tight px-4 leading-relaxed">
                                No remote peers detected. The matrix is currently silent.
                            </div>
                        </div>
                    ) : (
                        <div className="grid grid-cols-1 gap-2">
                            {peers.map((p) => (
                                <div key={p.peer_id} className="flex items-center justify-between px-4 py-3 bg-tertiary/20 rounded-xl border border-glass-border/30 hover:bg-tertiary/40 transition-smooth group">
                                    <div className="flex items-center gap-3 min-w-0">
                                        <div className="w-1.5 h-1.5 rounded-full bg-accent-emerald shadow-[0_0_8px_rgba(16,185,129,0.5)] group-hover:scale-125 transition-smooth"></div>
                                        <span className="text-[11px] font-mono text-primary/60 truncate">{p.peer_id}</span>
                                    </div>
                                    <span className="text-[9px] text-muted font-black uppercase opacity-0 group-hover:opacity-100 transition-smooth">LIVE</span>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            </div>

            {/* ── Intelligence Layer ── */}
            {(mode === "scout" || mode === "scout-initializing" || webLLMError) && (
                <div className="flex flex-col gap-5 mt-auto pt-6 border-t border-glass-border">
                    <div className="text-[10px] uppercase tracking-[0.25em] text-muted font-black flex items-center gap-2.5">
                        <div className="w-1.5 h-1.5 rounded-full bg-accent-blue shadow-glow-cyan/20"></div>
                        Intel Layer
                    </div>

                    <div className="bg-accent-blue/5 rounded-2xl p-5 border border-accent-blue/20 flex flex-col gap-4 relative overflow-hidden group">
                        <div className="absolute top-0 right-0 w-24 h-24 bg-accent-blue/5 rounded-full -mr-12 -mt-12 blur-3xl group-hover:bg-accent-blue/10 transition-smooth"></div>

                        {webLLMError ? (
                            <div className="text-[11px] text-accent-rose font-medium leading-relaxed italic">{webLLMError}</div>
                        ) : webLLMProgress ? (
                            <div className="flex flex-col gap-3">
                                <div className="flex justify-between items-center text-[11px]">
                                    <span className="text-accent-blue font-bold uppercase tracking-widest">Scout Initializing</span>
                                    <span className="text-white font-black tabular-nums">{Math.round(webLLMProgress.progress * 100)}%</span>
                                </div>
                                <div className="h-1 w-full bg-accent-blue/10 rounded-full overflow-hidden">
                                    <div
                                        className="h-full bg-accent-blue shadow-glow-cyan transition-all duration-300"
                                        style={{ width: `${webLLMProgress.progress * 100}%` }}
                                    ></div>
                                </div>
                                <div className="text-[9px] text-accent-blue/60 font-medium leading-tight line-clamp-2 italic">{webLLMProgress.text}</div>
                            </div>
                        ) : (
                            <div className="flex flex-col gap-3">
                                <div className="flex justify-between items-center">
                                    <span className="text-[11px] text-accent-emerald font-black uppercase tracking-widest">Active Contributor</span>
                                    <div className="flex gap-1">
                                        <div className="w-1 h-3 bg-accent-emerald/20 rounded-full animate-[pulse_1s_infinite]"></div>
                                        <div className="w-1 h-3 bg-accent-emerald/40 rounded-full animate-[pulse_1.2s_infinite]"></div>
                                        <div className="w-1 h-3 bg-accent-emerald/60 rounded-full animate-[pulse_1.4s_infinite]"></div>
                                    </div>
                                </div>
                                <p className="text-[10px] text-secondary/80 leading-relaxed font-medium">
                                    Accelerating the Shard network via WebGPU draft generation. Enhanced priority enabled.
                                </p>
                            </div>
                        )}
                    </div>
                </div>
            )}
        </aside>
    )
}
