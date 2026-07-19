import { RouterProvider } from "react-router-dom";

import { AppProviders } from "@/app/providers";
import { router } from "@/app/router";
import { AdminAuthGate } from "@/features/admin-auth";

export function App() {
  return (
    <AppProviders>
      <AdminAuthGate>
        <RouterProvider router={router} />
      </AdminAuthGate>
    </AppProviders>
  );
}
