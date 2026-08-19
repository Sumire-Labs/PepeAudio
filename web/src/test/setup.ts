import { vi } from "vitest";

vi.mock("@stylexjs/stylex", async (importOriginal) => {
  const stylex = await importOriginal<typeof import("@stylexjs/stylex")>();
  return {
    ...stylex,
    create: <T extends Record<string, unknown>>(styles: T): T =>
      Object.fromEntries(
        Object.keys(styles).map((name) => [name, { $$css: true }])
      ) as T
  };
});

HTMLCanvasElement.prototype.getContext = vi.fn(() => ({
  arc: vi.fn(),
  beginPath: vi.fn(),
  stroke: vi.fn(),
  globalAlpha: 1,
  lineCap: "round",
  lineWidth: 1,
  strokeStyle: ""
})) as unknown as typeof HTMLCanvasElement.prototype.getContext;

Object.defineProperty(window, "matchMedia", {
  configurable: true,
  writable: true,
  value: vi.fn((query: string): MediaQueryList => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(() => true)
  }))
});
