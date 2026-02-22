'use client'

import Header from "@/components/Header"
import ChatPanel from "@/components/ChatPanel"

export default function ChatPage() {
    return (
        <div className="container section">
            <Header />
            <main className="grid grid-2 mt-auto" style={{ height: "calc(100vh - 160px)" }}>
                <div className="card" style={{ gridColumn: "1 / -1", height: "100%", padding: 0, overflow: "hidden", display: "flex", flexDirection: "column" }}>
                    <ChatPanel mode="scout" />
                </div>
            </main>
        </div>
    )
}
