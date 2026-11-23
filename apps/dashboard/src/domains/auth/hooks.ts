"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRouter } from "next/navigation";

import { toast } from "sonner";

import { authClient, dashboardPath } from "./client";
import { API_ROUTES } from "@/config/api";

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
    mutationFn: async (input: { email: string; name?: string }) => {
      const response = await authClient.signIn.magicLink({
        email: input.email,
        name: input.name,
        callbackURL: dashboardPath("/onboarding"),
        newUserCallbackURL: dashboardPath("/onboarding"),
        errorCallbackURL: dashboardPath("/auth/signup"),
      });
      if (response.error) {
        throw toAuthError(response.error, "Sign up failed");
      }
      return response.data;
    },
    onSuccess: (_data, variables) => {
      toast.success("Check your inbox for a sign-in link");
      router.push(`/auth/verify-email?email=${encodeURIComponent(variables.email)}`);
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });
}

export function useEmailSignIn() {
  const router = useRouter();
  return useMutation({
    mutationFn: async (input: { email: string }) => {
      const response = await authClient.signIn.magicLink({
        email: input.email,
        callbackURL: dashboardPath("/console/overview"),
        newUserCallbackURL: dashboardPath("/onboarding"),
        errorCallbackURL: dashboardPath("/auth/login"),
      });
      if (response.error) {
        throw toAuthError(response.error, "Sign in failed");
      }
      return response.data;
    },
    onSuccess: (_data, variables) => {
      toast.success("Check your inbox for a sign-in link");
      router.push(`/auth/verify-email?email=${encodeURIComponent(variables.email)}`);
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
        newUserCallbackURL: dashboardPath("/onboarding"),
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
      const response = await authClient.signIn.magicLink({
        email,
        callbackURL: dashboardPath("/console/overview"),
        newUserCallbackURL: dashboardPath("/onboarding"),
        errorCallbackURL: dashboardPath("/auth/verify-email"),
      });
      if (response.error) {
        throw toAuthError(response.error, "Could not resend the sign-in link");
      }
      return response.data;
    },
    onSuccess: () => {
      toast.success("Sign-in link sent — check your inbox");
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
      const response = await authClient.signOut();
      if (response.error) {
        throw toAuthError(response.error, "Sign out failed");
      }
      return response.data;
    },
    onMutate: () => {
      queryClient.setQueryData(SESSION_KEY, null);
      router.push("/auth/login");
    },
    onSuccess: () => {
      queryClient.clear();
      toast.success("Signed out");
    },
    onError: (error) => {
      void queryClient.invalidateQueries({ queryKey: SESSION_KEY });
      toast.error(error.message);
    },
  });
}

type DeleteUserResponse = { success?: boolean; message?: string };

async function requestDeleteAccount(body: { token?: string } = {}) {
  const response = await fetch(`${API_ROUTES.auth}/delete-user`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    credentials: "include",
    body: JSON.stringify(body),
  });
  const data = (await response.json().catch(() => null)) as DeleteUserResponse | null;
  if (!response.ok) {
    throw new Error(data?.message ?? "Account deletion request failed");
  }
  if (!data?.success) {
    throw new Error(data?.message ?? "Account deletion request failed");
  }
  return data;
}

export function useDeleteAccount() {
  return useMutation({
    mutationFn: () => requestDeleteAccount(),
    onSuccess: () => {
      toast.success("Deletion confirmation email sent — check your inbox");
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });
}

export function useConfirmAccountDeletion() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (token: string) => requestDeleteAccount({ token }),
    onSuccess: () => {
      queryClient.clear();
    },
  });
}
