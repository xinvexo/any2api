import { OAuthAccounts } from "./OAuthAccounts";
import { OAuthLogin } from "./OAuthLogin";

export function OAuthManagement() {
  return (
    <div className="space-y-7">
      <OAuthLogin />
      <OAuthAccounts />
    </div>
  );
}
