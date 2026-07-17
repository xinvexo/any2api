import { ArrowLeft } from "lucide-react";
import { Link } from "react-router-dom";

export function NotFoundPage() {
  return (
    <div className="py-16 text-center">
      <p className="text-sm font-medium text-accent-copy">404</p>
      <h1 className="mt-3 text-3xl font-semibold">页面不存在</h1>
      <Link
        to="/"
        className="focus-ring mt-7 inline-flex h-10 items-center gap-2 rounded-control border border-subtle bg-surface px-4 text-sm font-semibold shadow-hairline hover:bg-surface-hover"
      >
        <ArrowLeft size={16} />
        返回总览
      </Link>
    </div>
  );
}
