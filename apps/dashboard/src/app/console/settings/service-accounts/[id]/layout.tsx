import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Service Account | Argus Router",
  description: "Service account details and logs.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

