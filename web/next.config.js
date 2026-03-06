/** @type {import('next').NextConfig} */
// Default to server mode in hosted environments.
// Only enable static export when explicitly requested.
const isTauri = process.env.TAURI_ENV_PLATFORM !== undefined;
const staticExport = process.env.NEXT_OUTPUT_MODE === "export" || isTauri;
const extraConnectSources = (process.env.NEXT_PUBLIC_CONNECT_SRC_ALLOWLIST || "")
  .split(",")
  .map((item) => item.trim())
  .filter(Boolean);
const baseConnectSources = [
  "'self'",
  "https:",
  "wss:",
  "http://127.0.0.1:*",
  "http://localhost:*",
  "ws://127.0.0.1:*",
  "ws://localhost:*",
  "https://cloudflareinsights.com",
  ...extraConnectSources,
];
const benchmarkConnectSources = [...baseConnectSources, "http:", "ws:"];

function buildCsp(connectSources, { upgradeInsecureRequests = true } = {}) {
  const directives = [
    "default-src 'self'",
    "script-src 'self' 'unsafe-eval' 'wasm-unsafe-eval' 'unsafe-inline' blob: https://static.cloudflareinsights.com",
    "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com",
    "font-src 'self' data: https://fonts.gstatic.com",
    `connect-src ${connectSources.join(" ")}`,
    "img-src 'self' data: blob: https:",
    "worker-src 'self' blob:",
    "child-src 'self' blob:",
    "object-src 'none'",
    "base-uri 'self'",
    "form-action 'self'",
    "frame-ancestors 'none'",
  ];
  if (upgradeInsecureRequests) {
    directives.push("upgrade-insecure-requests");
  }
  return directives.join("; ");
}

const nextConfig = {
  reactStrictMode: true,
  output: staticExport ? "export" : undefined,
  typescript: {
    ignoreBuildErrors: true,
  },
  eslint: {
    ignoreDuringBuilds: true,
  },
  images: { unoptimized: true },
  experimental: {
    sri: { algorithm: 'sha256' },
  },
  ...(staticExport
    ? {}
    : {
      // Security headers are only emitted in server mode.
      async headers() {
        return [
          {
            source: "/benchmark/:path*",
            headers: [
              { key: "X-Frame-Options", value: "DENY" },
              { key: "X-Content-Type-Options", value: "nosniff" },
              { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
              { key: "X-DNS-Prefetch-Control", value: "on" },
              { key: "Strict-Transport-Security", value: "max-age=63072000; includeSubDomains; preload" },
              { key: "Permissions-Policy", value: "camera=(), microphone=(), geolocation=(), interest-cohort=()" },
              { key: "Content-Security-Policy", value: buildCsp(benchmarkConnectSources, { upgradeInsecureRequests: false }) },
            ],
          },
          {
            source: "/:path*",
            headers: [
              { key: "X-Frame-Options", value: "DENY" },
              { key: "X-Content-Type-Options", value: "nosniff" },
              { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
              { key: "X-DNS-Prefetch-Control", value: "on" },
              { key: "Strict-Transport-Security", value: "max-age=63072000; includeSubDomains; preload" },
              { key: "Permissions-Policy", value: "camera=(), microphone=(), geolocation=(), interest-cohort=()" },
              { key: "Content-Security-Policy", value: buildCsp(baseConnectSources) },
            ],
          },
        ];
      },
    }),
  webpack: (config, { isServer }) => {
    if (!isServer) {
      // Node.js polyfill fallbacks for libp2p browser usage
      config.resolve.fallback = {
        ...config.resolve.fallback,
        net: false,
        tls: false,
        fs: false,
        os: false,
        path: false,
        stream: false,
        crypto: false,
        http: false,
        https: false,
        zlib: false,
        dns: false,
        dgram: false,
        child_process: false,
      };
    }
    return config;
  },
};

module.exports = nextConfig;
