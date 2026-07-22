import { Construction } from "lucide-react";

import { Surface } from "@/shared/ui/Surface";

export function PlaceholderPage({ title }: { title: string }) {
  return (
    <>
      <h1 className="sr-only">{title}</h1>
      <Surface className="flex min-h-56 items-center justify-center p-7 text-center">
        <div>
          <Construction size={24} className="mx-auto text-tertiary" />
          <p className="mt-4 text-sm font-medium">模块边界已建立</p>
          <p className="mt-1 text-sm text-secondary">业务接口将在对应阶段接入</p>
        </div>
      </Surface>
    </>
  );
}
