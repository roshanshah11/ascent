import { Component, type ReactNode } from "react";

interface LazyPanelBoundaryProps {
  children: ReactNode;
  message: string;
}

interface LazyPanelBoundaryState {
  failed: boolean;
}

/** Keeps a failed optional visualization from taking down the workbench. */
export default class LazyPanelBoundary extends Component<
  LazyPanelBoundaryProps,
  LazyPanelBoundaryState
> {
  state: LazyPanelBoundaryState = { failed: false };

  static getDerivedStateFromError(): LazyPanelBoundaryState {
    return { failed: true };
  }

  render() {
    if (this.state.failed) {
      return (
        <div className="viewport-error" role="alert">
          {this.props.message}
        </div>
      );
    }
    return this.props.children;
  }
}
