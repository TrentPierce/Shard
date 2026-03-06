import fs from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import process from "node:process"
import { chromium } from "playwright-core"

function parseArgs(argv) {
  const args = {
    count: 1,
    pageBase: "http://127.0.0.1:3000/benchmark/scout",
    backends: [],
    channel: process.platform === "win32" ? "msedge" : "chrome",
    startupTimeoutMs: 900000,
    headless: false,
    userDataDir: process.env.SHARD_BROWSER_SCOUT_PROFILE_DIR || "",
    reuseProfile: false,
  }

  for (let i = 0; i < argv.length; i += 1) {
    const key = argv[i]
    const value = argv[i + 1]
    if (key === "--count") args.count = Number(value || 1)
    if (key === "--page-base") args.pageBase = value || args.pageBase
    if (key === "--backends") args.backends = String(value || "").split(",").map((item) => item.trim()).filter(Boolean)
    if (key === "--channel") args.channel = value || args.channel
    if (key === "--startup-timeout-ms") args.startupTimeoutMs = Number(value || args.startupTimeoutMs)
    if (key === "--headless") args.headless = String(value || "false").toLowerCase() === "true"
    if (key === "--user-data-dir") args.userDataDir = value || args.userDataDir
    if (key === "--reuse-profile") args.reuseProfile = String(value || "false").toLowerCase() === "true"
  }
  return args
}

function benchmarkUrl(pageBase, backend, slot) {
  const url = new URL(pageBase)
  if (backend) url.searchParams.set("backend", backend)
  url.searchParams.set("slot", String(slot))
  url.searchParams.set("label", `Benchmark Scout ${slot + 1}`)
  return url.toString()
}

async function waitForContribution(page, timeoutMs) {
  const locator = page.locator("[data-benchmark-scout-root]")
  await locator.waitFor({ state: "visible", timeout: timeoutMs })
  await page.waitForFunction(
    () => {
      const root = document.querySelector("[data-benchmark-scout-root]")
      const state = root?.getAttribute("data-scout-state") || ""
      return state === "contributing" || state === "failed"
    },
    undefined,
    { timeout: timeoutMs },
  )
  const state = await locator.getAttribute("data-scout-state")
  if (state !== "contributing") {
    const text = await locator.textContent()
    throw new Error(`benchmark scout failed to start: ${text || "unknown page state"}`)
  }
}

function attachPageLogging(page, slot) {
  page.on("console", (msg) => console.log(`[browser-scout ${slot}] ${msg.type()}: ${msg.text()}`))
  page.on("pageerror", (error) =>
    console.error(`[browser-scout ${slot}] pageerror: ${error instanceof Error ? error.stack || error.message : String(error)}`),
  )
  page.on("requestfailed", (request) =>
    console.warn(`[browser-scout ${slot}] requestfailed: ${request.method()} ${request.url()} ${request.failure()?.errorText || "unknown"}`),
  )
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  if (!Number.isFinite(args.count) || args.count <= 0) {
    throw new Error("--count must be >= 1")
  }

  const defaultProfileRoot = path.join(os.tmpdir(), `shard-browser-scout-profiles-${args.channel}`)
  const profileRoot = path.resolve(args.userDataDir || defaultProfileRoot)
  await fs.mkdir(profileRoot, { recursive: true })
  const userDataDir = args.reuseProfile
    ? profileRoot
    : await fs.mkdtemp(path.join(profileRoot, "run-"))
  const ephemeralProfile = !args.reuseProfile
  console.log(
    `[browser-scout-runner] launching channel=${args.channel} headless=${args.headless} profile=${userDataDir} reuse=${args.reuseProfile}`,
  )
  const context = await chromium.launchPersistentContext(userDataDir, {
    channel: args.channel,
    headless: args.headless,
    viewport: { width: 1280, height: 900 },
    args: [
      "--enable-unsafe-webgpu",
      "--ignore-gpu-blocklist",
      "--disable-features=CalculateNativeWinOcclusion",
      "--disable-dawn-features=disallow_unsafe_apis",
    ],
  })

  const cleanup = async () => {
    await context.close().catch(() => undefined)
    if (ephemeralProfile) {
      await fs.rm(userDataDir, { recursive: true, force: true }).catch(() => undefined)
    }
  }

  let shuttingDown = false
  const handleSignal = async (signal) => {
    if (shuttingDown) return
    shuttingDown = true
    console.log(`[browser-scout-runner] shutting down on ${signal}`)
    await cleanup()
    process.exit(0)
  }
  process.on("SIGINT", handleSignal)
  process.on("SIGTERM", handleSignal)

  try {
    const pages = []
    const warmBackend = args.backends[0] || ""
    console.log(`[browser-scout-runner] opening warm page backend=${warmBackend || "relative api"}`)
    const firstPage = await context.newPage()
    attachPageLogging(firstPage, 0)
    await firstPage.goto(benchmarkUrl(args.pageBase, warmBackend, 0), { waitUntil: "domcontentloaded" })
    console.log("[browser-scout-runner] warm page loaded, waiting for contribution state")
    await waitForContribution(firstPage, args.startupTimeoutMs)
    console.log(`[browser-scout-runner] scout 0 contributing via ${warmBackend || "relative api"}`)
    pages.push(firstPage)

    for (let slot = 1; slot < args.count; slot += 1) {
      const backend = args.backends[slot % Math.max(1, args.backends.length)] || warmBackend
      const page = await context.newPage()
      attachPageLogging(page, slot)
      await page.goto(benchmarkUrl(args.pageBase, backend, slot), { waitUntil: "domcontentloaded" })
      await waitForContribution(page, args.startupTimeoutMs)
      console.log(`[browser-scout-runner] scout ${slot} contributing via ${backend || "relative api"}`)
      pages.push(page)
    }

    console.log(`[browser-scout-runner] ready count=${pages.length}`)
    await new Promise(() => {})
  } catch (error) {
    await cleanup()
    throw error
  }
}

main().catch((error) => {
  console.error(`[browser-scout-runner] fatal: ${error instanceof Error ? error.stack || error.message : String(error)}`)
  process.exit(1)
})
