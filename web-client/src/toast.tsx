import { createContext, useCallback, useContext, useRef, useState, type ReactNode } from 'react';

interface Toast {
  id: number;
  message: string;
  kind: 'info' | 'error';
}

const ToastContext = createContext<(message: string, kind?: 'info' | 'error') => void>(() => {});

export function useToast(): (message: string, kind?: 'info' | 'error') => void {
  return useContext(ToastContext);
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);

  const push = useCallback((message: string, kind: 'info' | 'error' = 'info') => {
    const id = nextId.current++;
    setToasts((prev) => [...prev, { id, message, kind }]);
    setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 3200);
  }, []);

  return (
    <ToastContext.Provider value={push}>
      {children}
      <div className="pointer-events-none fixed inset-x-0 bottom-6 z-50 flex flex-col items-center gap-2 px-4">
        {toasts.map((t) => (
          <div
            key={t.id}
            className="glass-strong pointer-events-auto max-w-md rounded-2xl px-4 py-2.5 text-sm text-[var(--text)] shadow-xl fade-in"
            style={{ boxShadow: '0 12px 40px var(--shadow)' }}
          >
            {t.message}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
