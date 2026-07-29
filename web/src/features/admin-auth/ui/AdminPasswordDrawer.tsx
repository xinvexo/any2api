import { AdminPasswordRotation } from "./AdminPasswordRotation";
import { SideDrawer } from "@/shared/ui/SideDrawer";

export function AdminPasswordDrawer({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  return (
    <SideDrawer
      open={open}
      title="修改密码"
      description="更新后，其他已登录浏览器需要重新登录。"
      onClose={onClose}
    >
      <AdminPasswordRotation onCancel={onClose} />
    </SideDrawer>
  );
}
