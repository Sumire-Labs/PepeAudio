// SPDX-License-Identifier: Apache-2.0
import type { NextConfig } from "next";

const backend = process.env.BACKEND_ORIGIN ?? "http://localhost:5080";

// Same-origin proxy: keeps the session cookie and SignalR handshake first-party.
const nextConfig: NextConfig = {
  output: "standalone",
  images: { unoptimized: true },
  async rewrites() {
    return [
      { source: "/api/:path*", destination: `${backend}/api/:path*` },
      { source: "/hubs/:path*", destination: `${backend}/hubs/:path*` },
    ];
  },
};

export default nextConfig;
