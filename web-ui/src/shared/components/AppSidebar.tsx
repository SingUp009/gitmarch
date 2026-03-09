"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { GitBranch, Home, Users, Settings } from "lucide-react";
import { cn } from "shared/lib/utils";

const navItems = [
  { href: "/",         icon: Home,      label: "ダッシュボード" },
  { href: "/repos",    icon: GitBranch, label: "リポジトリ" },
  { href: "/users",    icon: Users,     label: "ユーザー" },
  { href: "/settings", icon: Settings,  label: "設定" },
] as const;

export function AppSidebar() {
  const pathname = usePathname();

  return (
    <aside className="w-60 h-screen flex flex-col bg-sidebar border-r border-sidebar-border shrink-0">
      {/* ロゴ */}
      <div className="h-14 flex items-center gap-2.5 px-4 border-b border-sidebar-border">
        <GitBranch className="w-5 h-5 text-sidebar-primary" />
        <span className="font-semibold text-sidebar-foreground tracking-tight">
          gitmarch
        </span>
      </div>

      {/* ナビゲーション */}
      <nav className="flex-1 px-2 py-3 space-y-0.5">
        {navItems.map(({ href, icon: Icon, label }) => {
          const isActive = pathname === href;
          return (
            <Link
              key={href}
              href={href}
              className={cn(
                "flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors",
                isActive
                  ? "bg-sidebar-primary/15 text-sidebar-primary font-medium"
                  : "text-sidebar-foreground/60 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
              )}
            >
              <Icon className="w-4 h-4 shrink-0" />
              {label}
            </Link>
          );
        })}
      </nav>

      {/* フッター */}
      <div className="px-4 py-3 border-t border-sidebar-border">
        <p className="text-xs text-sidebar-foreground/40">gitmarch v0.1.0</p>
      </div>
    </aside>
  );
}
