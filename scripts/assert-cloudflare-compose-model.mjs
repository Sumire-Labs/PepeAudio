import { readFileSync } from "node:fs";

const source = process.argv[2];
if (!source) {
  throw new Error(
    "usage: node scripts/assert-cloudflare-compose-model.mjs <compose-model.json>",
  );
}

const model = JSON.parse(readFileSync(source, "utf8"));

function assert(condition, message) {
  if (!condition) {
    throw new Error(`invalid Cloudflare Tunnel Compose model: ${message}`);
  }
}

function service(name) {
  const value = model.services?.[name];
  assert(value, `service ${name} is missing`);
  return value;
}

function assertExact(actual, expected, label) {
  const left = [...actual].sort();
  const right = [...expected].sort();
  assert(
    JSON.stringify(left) === JSON.stringify(right),
    `${label} differs: ${left.join(", ")}`,
  );
}

const caddy = service("caddy");

assert(
  model.services?.cloudflared === undefined,
  "cloudflared must remain a host-managed systemd service",
);
assertExact(caddy.profiles, ["production"], "Caddy profiles");
assertExact(Object.keys(caddy.networks ?? {}), ["edge"], "Caddy networks");
assert(caddy.user === "10001:10001", "Tunnel Caddy must run as a non-root user");
assert(
  JSON.stringify(caddy.command) ===
    JSON.stringify([
      "caddy",
      "run",
      "--config",
      "/etc/caddy/Caddyfile.tunnel",
      "--adapter",
      "caddyfile",
    ]),
  "Caddy must use the internal HTTP-only tunnel configuration",
);

const ports = caddy.ports ?? [];
assert(ports.length === 1, "Caddy must publish exactly one loopback port");
const port = ports[0];
assert(typeof port === "object", "Caddy port must use the normalized long form");
assert(String(port.target) === "8080", "Caddy container port must be 8080");
assert(String(port.published) === "18080", "Caddy host port must be 18080");
assert(port.host_ip === "127.0.0.1", "Caddy must bind only to IPv4 loopback");
assert((port.protocol ?? "tcp") === "tcp", "Caddy port must use TCP");

assert(caddy.read_only === true, "Caddy root filesystem must be read-only");
assertExact(caddy.cap_drop ?? [], ["ALL"], "Caddy dropped capabilities");
assert(
  (caddy.cap_add ?? []).length === 0,
  "Caddy must not retain low-port capabilities",
);
assert(
  (caddy.security_opt ?? []).includes("no-new-privileges:true"),
  "Caddy must set no-new-privileges",
);
assert((caddy.volumes ?? []).length === 0, "Caddy must not retain writable volumes");

for (const name of ["api", "bot", "postgres", "valkey", "migrate"]) {
  assert(
    (service(name).ports ?? []).length === 0,
    `${name} must not publish host ports`,
  );
}

assert(
  model.networks?.tunnel === undefined,
  "a Docker tunnel network is unnecessary with host cloudflared",
);
assert(
  model.secrets?.cloudflare_tunnel_token === undefined,
  "the host cloudflared token must not enter the Compose model",
);

console.log("Host-managed Cloudflare Tunnel Compose assertions passed.");
