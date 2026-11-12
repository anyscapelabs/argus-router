"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRouter } from "next/navigation";

import { toast } from "sonner";

import { authClient, dashboardPath } from "./client";

const SESSION_KEY = ["auth", "session"] as const;

export type SessionData = Awaited<ReturnType<typeof authClient.getSession>>["data"];
export type AuthError = { code?: string; message?: string; status?: number };

function toAuthError(
  error: AuthError | null,
  fallback: string,
): Error {
  return new Error(error?.message ?? fallback);
}

export function useSession() {
  return useQuery({
    queryKey: SESSION_KEY,
    queryFn: async () => {
      const response = await authClient.getSession();
      if (response.error) {
        if (response.error.status === 401 || response.error.status === 403) {
          return null;
        }
        throw toAuthError(response.error, "Failed to load session");
      }
      return response.data;
    },
  });
}

export function useEmailSignUp() {
  const router = useRouter();
  return useMutation({
    mutationFn: async (input: { email: string; password: string; name: string }) => {
      const response = await authClient.signUp.email(input);
      if (response.error) {
        throw toAuthError(response.error, "Sign up failed");
      }
      return response.data;
    },
    onSuccess: () => {
      toast.success("Account created — check your inbox to verify your email");
      router.push("/auth/verify-email");
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });
}

export function useEmailSignIn() {
  const router = useRouter();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (input: { email: string; password: string }) => {
      const response = await authClient.signIn.email(input);
      if (response.error) {
        throw toAuthError(response.error, "Sign in failed");
      }
      return response.data;
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: SESSION_KEY });
      toast.success("Signed in");
      router.push("/console/overview");
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });
}

export function useSocialSignIn(provider: "google" | "github", callbackPath = "/console/overview") {
  return useMutation({
    mutationFn: async () => {
      const response = await authClient.signIn.social({
        provider,
        callbackURL: dashboardPath(callbackPath),
      });
      if (response.error) {
        throw toAuthError(response.error, "Social sign-in failed");
      }
      return response.data;
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });
}

export function useVerifyEmail() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (token: string) => {
      const response = await authClient.verifyEmail({ query: { token } });
      if (response.error) {
        throw toAuthError(response.error, "Email verification failed");
      }
      return response.data;
    },
    onSuccess: () => {
      toast.success("Your email is verified");
      void queryClient.invalidateQueries({ queryKey: SESSION_KEY });
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });
}

export function useResendVerification() {
  return useMutation({
    mutationFn: async (email: string) => {
      const response = await authClient.sendVerificationEmail({
        email,
        callbackURL: dashboardPath("/auth/verify-email"),
      });
      if (response.error) {
        throw toAuthError(response.error, "Could not resend the verification email");
      }
      return response.data;
    },
    onSuccess: () => {
      toast.success("Verification email sent — check your inbox");
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });
}

export function useSignOut() {
  const queryClient = useQueryClient();
  const router = useRouter();
  return useMutation({
    mutationFn: async () => {
      await authClient.signOut();
    },
    onSuccess: () => {
      queryClient.setQueryData(SESSION_KEY, null);
      queryClient.clear();
      toast.success("Signed out");
      router.push("/auth/login");
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });
}
