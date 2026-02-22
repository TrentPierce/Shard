"use client"

import { useAppContext } from "@/lib/context"
import { Terminal, Briefcase } from "lucide-react"

export function ThemeToggle() {
    const { theme, toggleTheme } = useAppContext()

    const isEnterprise = theme === "enterprise"

    return (
        <button
            onClick={toggleTheme}
            className="group flex items-center gap-2 px-3 py-1.5 glass-card hover:bg-white/5 transition-all"
            aria-label={isEnterprise ? "Switch to Developer Mode" : "Switch to Enterprise Mode"}
            title={isEnterprise ? "Switch to Developer Mode" : "Switch to Enterprise Mode"}
            type="button"
        >
            <div className="relative w-5 h-5">
                <Terminal
                    className={`absolute inset-0 transition-opacity duration-300 ${isEnterprise ? 'opacity-0' : 'opacity-100'}`}
                    size={20}
                />
                <Briefcase
                    className={`absolute inset-0 transition-opacity duration-300 ${isEnterprise ? 'opacity-100' : 'opacity-0'}`}
                    size={20}
                />
            </div>
            <span className="text-xs font-medium uppercase tracking-wider hidden md:inline">
                {isEnterprise ? "Enterprise" : "Terminal"}
            </span>
        </button>
    )
}
