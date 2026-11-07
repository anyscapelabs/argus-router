"use client";

import { Toaster } from "sonner";

export function ToastProvider({ children }: { children: React.ReactNode }) {
  return (
    <>
      {children}
      <Toaster
        position="top-right"
        closeButton
        toastOptions={{
          style: {
            background: "#161616",
            color: "#ededed",
            border: "1px solid #2a2a2a",
            borderRadius: "0",
            fontSize: "13px",
          },
        }}
      />
    </>
  );
}
