pub(super) const RESERVE_STATE: &str = r"
local clock = redis.call('TIME')
local now = tonumber(clock[1]) * 1000 + math.floor(tonumber(clock[2]) / 1000)
local ttl = tonumber(ARGV[2])
local capacity = tonumber(ARGV[4])
if not now or not ttl or not capacity then
  return redis.error_reply('invalid OAuth state capacity')
end
redis.call('ZREMRANGEBYSCORE', KEYS[2], '-inf', now)
local count = redis.call('ZCARD', KEYS[2])
if count >= capacity then
  return -1
end
if redis.call('SET', KEYS[1], ARGV[1], 'NX', 'PX', ARGV[2]) ~= false then
  redis.call('ZADD', KEYS[2], now + ttl, ARGV[5])
  redis.call('PEXPIRE', KEYS[2], ARGV[3])
  return 1
end
return 0
";

pub(super) const CONSUME_STATE: &str = r"
local value = redis.call('GET', KEYS[1])
if not value then
  return nil
end
redis.call('DEL', KEYS[1])
redis.call('ZREM', KEYS[2], ARGV[1])
if redis.call('ZCARD', KEYS[2]) == 0 then
  redis.call('DEL', KEYS[2])
end
return value
";

pub(super) const CREATE_SESSION: &str = r"
if redis.call('EXISTS', KEYS[1]) == 1 then
  return 0
end
local previous = redis.call('GET', KEYS[2])
redis.call('PSETEX', KEYS[1], ARGV[2], ARGV[1])
redis.call('PSETEX', KEYS[2], ARGV[3], ARGV[4])
if previous and previous ~= ARGV[4] then
  redis.call('DEL', ARGV[5] .. previous)
end
return 1
";

pub(super) const LOAD_AND_REFRESH_SESSION: &str = r"
local encoded = redis.call('GET', KEYS[1])
if not encoded then
  return nil
end
if redis.call('GET', KEYS[2]) ~= ARGV[1] then
  return nil
end
local ok, session = pcall(cjson.decode, encoded)
if not ok or session.schema_version ~= 1 then
  return redis.error_reply('invalid auth session')
end
local now = tonumber(ARGV[2])
local expires = tonumber(session.expires_at_ms)
local policy_expires = tonumber(ARGV[4])
if not now or not expires or not policy_expires then
  return redis.error_reply('invalid auth session lifetime')
end
local effective_expires = math.min(expires, policy_expires)
if effective_expires <= now then
  redis.call('DEL', KEYS[1])
  if redis.call('GET', KEYS[2]) == ARGV[1] then
    redis.call('DEL', KEYS[2])
  end
  return nil
end
local remaining = effective_expires - now
local idle = tonumber(ARGV[3])
local ttl = math.min(remaining, idle)
session.expires_at_ms = effective_expires
session.last_seen_at_ms = now
local refreshed = cjson.encode(session)
redis.call('PSETEX', KEYS[1], ttl, refreshed)
return refreshed
";

pub(super) const DESTROY_SESSION: &str = r"
redis.call('DEL', KEYS[1])
if redis.call('GET', KEYS[2]) == ARGV[1] then
  redis.call('DEL', KEYS[2])
end
return 1
";
