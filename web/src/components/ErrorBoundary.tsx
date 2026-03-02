"use client"

import React from "react"

type ErrorBoundaryProps = {
  children: React.ReactNode
}

type ErrorBoundaryState = {
  hasError: boolean
  error?: Error
}

export class ErrorBoundary extends React.Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error("UI error boundary caught:", error, info)
  }

  handleReload = () => {
    if (typeof window !== "undefined") {
      window.location.reload()
    }
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="error-boundary">
          <div className="error-boundary__panel">
            <p className="error-boundary__title">Dashboard encountered an error</p>
            <p className="error-boundary__detail">
              The UI failed to render. You can reload to reconnect to the node.
            </p>
            <button className="btn btn-primary" type="button" onClick={this.handleReload}>
              Reload Dashboard
            </button>
          </div>
          <style jsx>{`
            .error-boundary {
              min-height: 60vh;
              display: flex;
              align-items: center;
              justify-content: center;
              padding: 2rem;
            }
            .error-boundary__panel {
              max-width: 520px;
              width: 100%;
              border: 1px solid var(--ring);
              border-radius: 16px;
              padding: 24px;
              background: var(--base-900);
              box-shadow: var(--shadow-panel);
            }
            .error-boundary__title {
              font-size: 1.125rem;
              font-weight: 600;
              color: var(--ink-50);
              margin-bottom: 8px;
            }
            .error-boundary__detail {
              font-size: 0.9rem;
              color: var(--ink-300);
              margin-bottom: 16px;
            }
          `}</style>
        </div>
      )
    }

    return this.props.children
  }
}
