import { Component, type ErrorInfo, type ReactNode } from 'react';

/** Catches render errors so a component crash shows a recover card instead of a white screen. */
export class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error): { error: Error } {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error('Dashboard render error:', error, info);
  }

  render(): ReactNode {
    if (this.state.error) {
      return (
        <div className="grid h-full place-items-center p-6">
          <div className="glass-strong max-w-sm rounded-3xl p-8 text-center" style={{ boxShadow: '0 30px 80px var(--shadow)' }}>
            <h1 className="text-xl font-semibold">問題が発生しました</h1>
            <p className="mt-2 text-sm text-[var(--text-dim)]">画面の描画中にエラーが発生しました。再読み込みしてください。</p>
            <button onClick={() => window.location.reload()} className="mt-5 rounded-2xl accent-bg px-5 py-2.5 font-medium text-white transition hover:brightness-110">
              再読み込み
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
