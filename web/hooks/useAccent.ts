// SPDX-License-Identifier: Apache-2.0
"use client";
import { useEffect } from "react";

const DEFAULT_ACCENT: [number, number, number] = [250, 45, 85];

function applyAccent([r, g, b]: [number, number, number]): void {
  const root = document.documentElement.style;
  root.setProperty("--accent-r", String(r));
  root.setProperty("--accent-g", String(g));
  root.setProperty("--accent-b", String(b));
}

function normalizeAccent(r: number, g: number, b: number): [number, number, number] {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const lum = (max + min) / 2;
  if (max - min < 18 || lum < 24) return DEFAULT_ACCENT;
  const lift = lum < 90 ? 1.35 : 1;
  const clamp = (v: number) => Math.max(0, Math.min(255, Math.round(v * lift)));
  return [clamp(r), clamp(g), clamp(b)];
}

/** crossOrigin load avoids tainting the canvas; a CORS-blocked read throws and falls back to the default accent. */
export function useAccent(thumb?: string | null): void {
  useEffect(() => {
    if (!thumb) {
      applyAccent(DEFAULT_ACCENT);
      return;
    }
    let cancelled = false;
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      if (cancelled) return;
      try {
        const size = 12;
        const canvas = document.createElement("canvas");
        canvas.width = size;
        canvas.height = size;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;
        ctx.drawImage(img, 0, 0, size, size);
        const { data } = ctx.getImageData(0, 0, size, size); // throws if tainted
        let r = 0;
        let g = 0;
        let b = 0;
        let n = 0;
        for (let i = 0; i < data.length; i += 4) {
          if (data[i + 3]! < 200) continue;
          r += data[i]!;
          g += data[i + 1]!;
          b += data[i + 2]!;
          n++;
        }
        if (n === 0) return;
        applyAccent(normalizeAccent(r / n, g / n, b / n));
      } catch {
        applyAccent(DEFAULT_ACCENT); // tainted canvas (no CORS) — keep default
      }
    };
    img.onerror = () => applyAccent(DEFAULT_ACCENT);
    img.src = thumb;
    return () => {
      cancelled = true;
    };
  }, [thumb]);
}
