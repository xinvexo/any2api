import { Construction } from "lucide-react";

import { Surface } from "@/shared/ui/Surface";

export function PlaceholderPage({ title }: { title: string }) {
  return (
    <div className="space-y-7">
      <header>
        <p className="text-sm font-medium text-accent-copy">管理</p>
        <h1 className="mt-2 text-3xl font-semibold sm:text-[34px]">{title}</h1>
      </header>
      <Surface className="flex min-h-56 items-center justify-center p-7 text-center">
        <div>
          <Construction size={24} className="mx-auto text-tertiary" />
          <p className="mt-4 text-sm font-medium">模块边界已建立</p>
          <p className="mt-1 text-sm text-secondary">业务接口将在对应阶段接入</p>
        </div>
      </Surface>
    </div>
  );
}
