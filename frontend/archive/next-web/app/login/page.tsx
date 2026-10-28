"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Logo } from "@/components/Logo";

export default function LoginPage() {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    router.push("/");
  }

  return (
    <div className="min-h-screen bg-qoin-bg-primary flex items-center justify-center p-4">
      <div className="qoin-card w-full max-w-sm p-6">
        <div className="flex justify-center mb-6">
          <Logo size={40} />
        </div>
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <div>
            <label htmlFor="email" className="text-xs font-medium text-qoin-text-muted">
              Email
            </label>
            <input
              id="email"
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="qoin-focus mt-1 w-full rounded-lg bg-qoin-bg-primary border border-qoin-border px-3 py-2 text-sm text-qoin-text-primary"
            />
          </div>
          <div>
            <label htmlFor="password" className="text-xs font-medium text-qoin-text-muted">
              Password
            </label>
            <input
              id="password"
              type="password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="qoin-focus mt-1 w-full rounded-lg bg-qoin-bg-primary border border-qoin-border px-3 py-2 text-sm text-qoin-text-primary"
            />
          </div>
          <button
            type="submit"
            className="qoin-focus mt-2 rounded-lg bg-qoin-accent-cyan text-qoin-bg-primary font-semibold text-sm py-2.5 hover:opacity-90 transition-opacity"
          >
            Sign in
          </button>
        </form>
      </div>
    </div>
  );
}
