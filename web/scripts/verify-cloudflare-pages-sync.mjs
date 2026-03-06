import { execSync } from "node:child_process"
import path from "node:path"
import process from "node:process"

function run(command, cwd) {
  return execSync(command, {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    shell: true,
  }).trim()
}

const webDir = process.cwd()
const repoDir = path.resolve(webDir, "..")
const npxCommand = process.platform === "win32" ? "npx.cmd" : "npx"

const latestOriginMain = run("git rev-parse --short origin/main", repoDir)
const latestLocalHead = run("git rev-parse --short HEAD", repoDir)
const rawDeployments = run(
  `${npxCommand} wrangler pages deployment list --project-name shard --json`,
  webDir,
)
const deployments = JSON.parse(rawDeployments)
const production = deployments.find(
  (deployment) =>
    String(deployment.Environment || "").toLowerCase() === "production"
    && String(deployment.Branch || "") === "main",
)

if (!production) {
  console.error("Cloudflare Pages project 'shard' has no production deployment on branch 'main'.")
  process.exit(1)
}

const deployedSource = String(production.Source || "").trim()
const inSync = deployedSource === latestOriginMain || deployedSource === latestLocalHead

if (!inSync) {
  console.error(
    `Cloudflare Pages is behind origin/main. deployed=${deployedSource} origin/main=${latestOriginMain} local=${latestLocalHead}`,
  )
  process.exit(1)
}

console.log(
  `Cloudflare Pages project 'shard' is in sync with Git. deployed=${deployedSource}`,
)
