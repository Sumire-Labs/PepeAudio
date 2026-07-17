/**
 * Leading + trailing throttle. A burst of calls collapses to at most one
 * invocation per `ms` window, with a guaranteed trailing invocation after the
 * burst so the final state is never dropped. Used to coalesce player 'update'
 * bursts (e.g. a volume drag) into bounded realtime pushes.
 */
export function throttle(fn: () => void, ms: number): () => void {
  let lastInvoke = 0;
  let timer: NodeJS.Timeout | null = null;
  let pending = false;

  const invoke = (): void => {
    lastInvoke = Date.now();
    pending = false;
    fn();
  };

  return () => {
    const elapsed = Date.now() - lastInvoke;
    if (elapsed >= ms) {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
      invoke();
      return;
    }
    pending = true;
    if (!timer) {
      timer = setTimeout(() => {
        timer = null;
        if (pending) invoke();
      }, ms - elapsed);
    }
  };
}
