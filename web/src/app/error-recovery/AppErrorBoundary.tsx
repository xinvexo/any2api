import { Component, type ReactNode } from "react";

import { ErrorRecoveryPage } from "./ErrorRecoveryPage";

type Props = { children: ReactNode };
type State = { failed: boolean };

export class AppErrorBoundary extends Component<Props, State> {
  state: State = { failed: false };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  render() {
    if (this.state.failed) {
      return (
        <ErrorRecoveryPage
          title="管理界面发生错误"
          description="界面遇到无法恢复的错误。请重新加载后再试。"
        />
      );
    }
    return this.props.children;
  }
}
