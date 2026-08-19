import { useEffect, useState } from "react";

export function useClock(intervalMs = 500): number {
  const [nowUnixMs, setNowUnixMs] = useState(() => Date.now());

  useEffect(() => {
    const timer = window.setInterval(() => setNowUnixMs(Date.now()), intervalMs);
    return () => window.clearInterval(timer);
  }, [intervalMs]);

  return nowUnixMs;
}
