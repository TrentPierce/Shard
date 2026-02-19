import "@testing-library/jest-dom"

// Mock scrollIntoView which is not available in jsdom
window.HTMLElement.prototype.scrollIntoView = jest.fn()

// Mock ResizeObserver
global.ResizeObserver = jest.fn().mockImplementation(() => ({
  observe: jest.fn(),
  unobserve: jest.fn(),
  disconnect: jest.fn(),
}))

// Mock IntersectionObserver
global.IntersectionObserver = jest.fn().mockImplementation(() => ({
  observe: jest.fn(),
  unobserve: jest.fn(),
  disconnect: jest.fn(),
}))

// Mock Tauri internals for tests running in jsdom.
if (!window.__TAURI_INTERNALS__) {
  window.__TAURI_INTERNALS__ = {
    transformCallback: jest.fn(() => 0),
  }
}

jest.mock("@tauri-apps/api/event", () => ({
  listen: jest.fn(async () => jest.fn()),
}))

jest.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: jest.fn(async () => ({
    isFullscreen: jest.fn(async () => false),
    setFullscreen: jest.fn(async () => {}),
    close: jest.fn(async () => {}),
  })),
}))
