import type { LucideIcon } from "lucide-react";
import { Box, Braces, Cog, FileCode } from "lucide-react";

import { Badge } from "@/components/ui/badge";

/** DR-28：技术栈徽章——标志文件名 → 图标 + 配色（纯前端，零新依赖） */
const MAPPING: Record<string, { icon: LucideIcon; cls: string }> = {
  "package.json": { icon: Braces, cls: "text-yellow-600 bg-yellow-500/10" },
  "Cargo.toml": { icon: Cog, cls: "text-orange-600 bg-orange-500/10" },
  "go.mod": { icon: Box, cls: "text-cyan-600 bg-cyan-500/10" },
  "pyproject.toml": { icon: FileCode, cls: "text-blue-600 bg-blue-500/10" },
};

export function StackBadge({ flag }: { flag: string }) {
  const m = MAPPING[flag];
  if (!m) {
    return <Badge variant="outline">{flag}</Badge>;
  }
  const Icon = m.icon;
  return (
    <Badge variant="outline" className={m.cls}>
      <Icon className="mr-0.5 size-3" />
      {flag}
    </Badge>
  );
}
