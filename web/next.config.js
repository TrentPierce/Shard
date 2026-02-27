/** @type {import('next').NextConfig} */
// Default to server mode in hosted environments.
// Only enable static export when explicitly requested.
const isTauri = process.env.TAURI_ENV_PLATFORM !== undefined;
const staticExport = process.env.NEXT_OUTPUT_MODE === "export" || isTauri;

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
            source: "/:path*",
            headers: [
              { key: "X-Frame-Options", value: "DENY" },
              { key: "X-Content-Type-Options", value: "nosniff" },
              { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
              { key: "X-DNS-Prefetch-Control", value: "on" },
              { key: "Strict-Transport-Security", value: "max-age=63072000; includeSubDomains; preload" },
              { key: "Permissions-Policy", value: "camera=(), microphone=(), geolocation=(), interest-cohort=()" },
            ],
          },
        ]
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
}

module.exports = nextConfig
