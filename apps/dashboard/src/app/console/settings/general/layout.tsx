import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "General | Argus Router",
  description: "Workspace name and avatar settings.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

