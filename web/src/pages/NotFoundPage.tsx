import { ArrowLeft } from "lucide-react";
import { Link } from "react-router-dom";

import { buttonClassName } from "@/shared/ui/button-class-name";

export function NotFoundPage() {
  return (
    <div className="py-16 text-center">
      <p className="text-sm font-medium text-accent-copy">404</p>
      <h1 className="mt-3 text-3xl font-semibold">页面不存在</h1>
      <Link
        to="/"
        className={buttonClassName({ variant: "secondary", size: "lg", className: "mt-7" })}
      >
        <ArrowLeft size={16} />
        返回系统总览
      </Link>
    </div>
  );
}
