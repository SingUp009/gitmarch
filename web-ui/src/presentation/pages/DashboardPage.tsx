"use client";

import { BranchList } from "@/feature/git/branch/components";
import { useState } from "react";

export default function DashboardPage() {
  const [repository, setRepository] = useState("test");

  return (
    <div className="max-w-3xl space-y-6">
      <h1 className="text-2xl font-semibold tracking-tight text-foreground">
        ダッシュボード
      </h1>
      <BranchList repository={repository} />
    </div>
  );
}
