"use client"

import React, { Component, ErrorInfo, ReactNode } from "react"

interface ErrorBoundaryProps {
    children: ReactNode
    fallback?: ReactNode
}

interface ErrorBoundaryState {
    hasError: boolean
    error: Error | null
}

/**
 * Error Boundary Component
 * 
 * Catches JavaScript errors anywhere in the child component tree,
 * logs them, and displays a fallback UI.
 * 
 * Usage:
 * <ErrorBoundary fallback={<div>Something went wrong</div>}>
 *   <YourComponent />
 * </ErrorBoundary>
 */
export default class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
    state: ErrorBoundaryState = {
        hasError: false,
        error: null,
    }

    static getDerivedStateFromError(error: Error): ErrorBoundaryState {
        return {
            hasError: true,
            error,
        }
    }

    componentDidCatch(error: Error, errorInfo: ErrorInfo) {
        console.error("[ErrorBoundary] Caught error:", error)
        console.error("[ErrorBoundary] Error info:", errorInfo)

        // Log error details for debugging
        const errorData = {
            error: error.message,
            componentStack: errorInfo.componentStack,
            timestamp: new Date().toISOString(),
        }

        // Store error in sessionStorage for potential recovery
        if (typeof window !== "undefined") {
            try {
                const errors = JSON.parse(sessionStorage.getItem("shard-errors") || "[]")
                errors.push(errorData)
                sessionStorage.setItem("shard-errors", JSON.stringify(errors.slice(-20))) // Keep last 20 errors
            } catch {
                // Ignore sessionStorage errors
            }
        }
    }

    handleReset = () => {
        this.setState({ hasError: false, error: null })
        
        // Clear errors from sessionStorage
        if (typeof window !== "undefined") {
            sessionStorage.removeItem("shard-errors")
        }
        
        // Reload page to recover
        window.location.reload()
    }

    render() {
        if (this.state.hasError) {
            return (
                <div
                    className="error-boundary"
                    role="alert"
                    aria-live="assertive"
                    aria-labelledby="error-title"
                >
                    <div className="error-boundary__content">
                        <div className="error-boundary__icon" aria-hidden="true">
                            ⚠️
                        </div>
                        <h1 id="error-title" className="error-boundary__title">
                            Something went wrong
                        </h1>
                        <p className="error-boundary__message">
                            {this.state.error?.message || "An unexpected error occurred"}
                        </p>
                        <details className="error-boundary__details">
                            <summary>Error Details</summary>
                            <pre>
                                {this.state.error?.stack || "No stack trace available"}
                            </pre>
                        </details>
                        <div className="error-boundary__actions">
                            <button
                                onClick={this.handleReset}
                                className="error-boundary__button error-boundary__button--primary"
                                type="button"
                            >
                                Reload Application
                            </button>
                            <button
                                onClick={() => this.setState({ hasError: false })}
                                className="error-boundary__button error-boundary__button--secondary"
                                type="button"
                            >
                                Dismiss
                            </button>
                        </div>
                    </div>
                </div>
            )
        }

        return this.props.children
    }
}
