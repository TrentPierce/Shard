'use client'

import Header from "@/components/Header"
import ChatPanel from "@/components/ChatPanel"
import NetworkStatus from "@/components/NetworkStatus"
import { useAppContext } from "@/lib/context"

export default function ChatPage() {
    const { mode, topology, rustStatus, webLLMProgress, webLLMError } = useAppContext()

    return (
        <div className="chat-page-wrapper bg-grid">
            <div className="hero-bg" style={{ opacity: 0.4 }}>
                <div className="gradient-orb gradient-orb--primary" style={{ width: 600, height: 600, top: -200, right: -200 }} />
                <div className="gradient-orb gradient-orb--secondary" style={{ width: 400, height: 400, bottom: -100, left: -100 }} />
            </div>

            <Header />
            
            <main className="chat-main container">
                <div className="chat-layout-grid">
                    {/* Sidebar: Status & Swarm Info */}
                    <aside className="chat-sidebar animate-fade-in">
                        <NetworkStatus 
                            mode={mode}
                            topology={topology}
                            rustStatus={rustStatus}
                            webLLMProgress={webLLMProgress}
                            webLLMError={webLLMError}
                        />
                    </aside>

                    {/* Primary Chat Interface */}
                    <section className="chat-container animate-fade-in-up">
                        <div className="card chat-card-polishing">
                            <ChatPanel mode={mode} />
                        </div>
                    </section>
                </div>
            </main>

            <style jsx>{`
                .chat-page-wrapper {
                    height: calc(100vh - var(--header-height));
                    display: flex;
                    flex-direction: column;
                    position: relative;
                    overflow: hidden;
                }
                .chat-main {
                    flex: 1;
                    display: flex;
                    padding: var(--space-md);
                    position: relative;
                    z-index: 1;
                    overflow: hidden;
                }
                .chat-layout-grid {
                    display: grid;
                    grid-template-columns: 300px 1fr;
                    gap: var(--space-lg);
                    width: 100%;
                    height: 100%;
                }
                .chat-sidebar {
                    height: 100%;
                    overflow-y: auto;
                }
                .chat-container {
                    height: 100%;
                    min-width: 0;
                }
                .chat-card-polishing {
                    height: 100%;
                    padding: 0;
                    overflow: hidden;
                    border-radius: var(--radius-xl);
                    background: rgba(10, 10, 15, 0.8);
                    backdrop-filter: blur(20px);
                    border: 1px solid var(--border-hover);
                    box-shadow: var(--shadow-lg);
                }
                @media (max-width: 1024px) {
                    .chat-layout-grid {
                        grid-template-columns: 1fr;
                        max-height: none;
                    }
                    .chat-sidebar {
                        display: none; /* Hide sidebar on mobile for cleaner chat */
                    }
                    .chat-main {
                        padding: var(--space-md);
                    }
                }
            `}</style>
        </div>
    )
}
