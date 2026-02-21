"use client"

import { useEffect, useState } from "react"

type Theme = "dark" | "light"

export function ThemeToggle() {
    const [theme, setTheme] = useState<Theme>("dark")

    useEffect(() => {
        const savedTheme = localStorage.getItem("shard-theme")
        const systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches
        const initialTheme: Theme = savedTheme === "light" || savedTheme === "dark"
            ? savedTheme
            : (systemDark ? "dark" : "light")

        setTheme(initialTheme)
        document.documentElement.setAttribute("data-theme", initialTheme)
    }, [])

    const toggleTheme = () => {
        const newTheme: Theme = theme === "dark" ? "light" : "dark"
        setTheme(newTheme)
        document.documentElement.setAttribute("data-theme", newTheme)
        localStorage.setItem("shard-theme", newTheme)
    }

    const isLightTheme = theme === "light"

    return (
        <button
            onClick={toggleTheme}
            className="theme-toggle"
            aria-label={isLightTheme ? "Switch to dark theme" : "Switch to light theme"}
            aria-pressed={isLightTheme}
            type="button"
        >
            <span className="theme-toggle__icon" aria-hidden="true">
                {isLightTheme ? "☀️" : "🌙"}
            </span>
        </button>
    )
}
