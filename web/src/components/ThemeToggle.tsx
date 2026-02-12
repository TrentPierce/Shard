"use client"

import { useEffect } from "react"

export function ThemeToggle() {
    useEffect(() => {
        // Load theme from localStorage or use system preference
        const savedTheme = localStorage.getItem("shard-theme")
        const systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches
        const theme = savedTheme || (systemDark ? "dark" : "light")
        document.documentElement.setAttribute("data-theme", theme)
    }, [])

    const toggleTheme = () => {
        const currentTheme = document.documentElement.getAttribute("data-theme") || "dark"
        const newTheme = currentTheme === "dark" ? "light" : "dark"
        document.documentElement.setAttribute("data-theme", newTheme)
        localStorage.setItem("shard-theme", newTheme)
    }

    return (
        <button
            onClick={toggleTheme}
            className="theme-toggle"
            aria-label="Toggle theme"
            type="button"
        >
            <span className="theme-toggle__icon" aria-hidden="true">
                🌙
            </span>
        </button>
    )
}
