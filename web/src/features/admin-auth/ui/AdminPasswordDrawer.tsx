import { AdminPasswordRotation } from "./AdminPasswordRotation";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { useAdminAuth } from "../model/use-admin-auth";

export function AdminPasswordDrawer({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const { submitting } = useAdminAuth();
  const close = () => {
    if (!submitting) {
      onClose();
    }
  };

  return (
    <SideDrawer
      open={open}
      title="修改密码"
      description="更新后，其他已登录浏览器需要重新登录。"
      onClose={close}
    >
      <AdminPasswordRotation onCancel={close} />
    </SideDrawer>
  );
}
