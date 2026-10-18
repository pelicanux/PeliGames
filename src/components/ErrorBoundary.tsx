import { Component, ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  render() {
    if (this.state.hasError) {
      return (
        <div role="alert" style={{ padding: "2rem", boxSizing: "border-box", color: "#e2e8f0", backgroundColor: "#0d111a", height: "100vh", overflow: "auto" }}>
          <h1>{document.documentElement.lang.startsWith("en") ? "The interface encountered an error." : "Ocorreu um erro na interface."}</h1>
          <button type="button" className="btn btn-primary" onClick={() => window.location.reload()}>
            {document.documentElement.lang.startsWith("en") ? "Reload interface" : "Recarregar interface"}
          </button>
          <details style={{ marginTop: "1.5rem" }}>
            <summary>{document.documentElement.lang.startsWith("en") ? "Technical details" : "Detalhes técnicos"}</summary>
            <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{this.state.error?.toString()}</pre>
            <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{this.state.error?.stack}</pre>
          </details>
        </div>
      );
    }
    return this.props.children;
  }
}
