import { ErrorRecoveryPage } from "./ErrorRecoveryPage";

export function RouteErrorPage() {
  return (
    <ErrorRecoveryPage
      title="页面加载失败"
      description="页面组件未能加载，应用更新后的资源可能已经变化。请重新加载后再试。"
    />
  );
}
