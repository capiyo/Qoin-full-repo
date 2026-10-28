"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { Logo } from "./Logo";

const NAV = [
  { href: "/", label: "Dashboard" },
  { href: "/positions", label: "Positions" },
  { href: "/settings", label: "Settings" },
];

export function Shell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  return (
    <div className="min-h-screen bg-qoin-bg-primary text-qoin-text-primary flex">
      <aside className="w-56 shrink-0 border-r border-qoin-border p-4 hidden md:flex md:flex-col gap-6">
        <Logo />
        <nav className="flex flex-col gap-1">
          {NAV.map((item) => {
            const active = pathname === item.href;
            return (
              <Link
                key={item.href}
                href={item.href}
                className={`qoin-focus rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
                  active
                    ? "bg-qoin-bg-secondary text-qoin-accent-cyan"
                    : "text-qoin-text-muted hover:text-qoin-text-primary hover:bg-qoin-bg-secondary"
                }`}
              >
                {item.label}
              </Link>
            );
          })}
        </nav>
      </aside>
      <div className="flex-1 flex flex-col min-w-0">
        <header className="md:hidden flex items-center justify-between border-b border-qoin-border p-4">
          <Logo size={28} />
        </header>
        <main className="flex-1 p-4 md:p-6">{children}</main>
      </div>
    </div>
  );
}
