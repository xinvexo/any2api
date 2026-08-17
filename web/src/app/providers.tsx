import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState, type PropsWithChildren } from "react";

import { AdminAuthProvider, useAdminAuth } from "@/features/admin-auth";
import { AdminRealtimeProvider } from "@/shared/realtime";

export function AppProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            refetchOnWindowFocus: false,
            retry: 1,
            staleTime: 10_000,
          },
        },
      }),
  );

  return (
    <QueryClientProvider client={queryClient}>
      <AdminAuthProvider>
        <AuthenticatedRealtimeProvider>{children}</AuthenticatedRealtimeProvider>
      </AdminAuthProvider>
    </QueryClientProvider>
  );
}

function AuthenticatedRealtimeProvider({ children }: PropsWithChildren) {
  const { session, refresh } = useAdminAuth();
  return (
    <AdminRealtimeProvider
      authenticated={session?.authenticated === true}
      onAuthRefresh={refresh}
    >
      {children}
    </AdminRealtimeProvider>
  );
}
