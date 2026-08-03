import { RouterProvider } from "react-router-dom";

import { AppErrorBoundary } from "@/app/error-recovery/AppErrorBoundary";
import { AppProviders } from "@/app/providers";
import { router } from "@/app/router";
import { AdminAuthGate } from "@/features/admin-auth";
import { ApplicationUpdateProvider } from "@/features/application-update";
import { NotificationHost } from "@/shared/notifications";

export function App() {
  return (
    <AppErrorBoundary>
      <AppProviders>
        <ApplicationUpdateProvider>
          <AdminAuthGate>
            <RouterProvider router={router} />
          </AdminAuthGate>
        </ApplicationUpdateProvider>
        {/* Global feedback viewport — outside the page router so menu switches keep toasts. */}
        <NotificationHost />
      </AppProviders>
    </AppErrorBoundary>
  );
}
