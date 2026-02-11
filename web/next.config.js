/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
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
