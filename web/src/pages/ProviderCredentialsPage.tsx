import { Navigate } from "react-router-dom";

/** Legacy deep link; credentials are nested under /providers. */
export function ProviderCredentialsPage() {
  return <Navigate to="/providers" replace />;
}
