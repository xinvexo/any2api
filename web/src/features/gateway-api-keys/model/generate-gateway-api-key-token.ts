/** Must match domain `GATEWAY_TOKEN_PREFIX` / `GATEWAY_TOKEN_BODY_LEN`. */
export const GATEWAY_TOKEN_PREFIX = "sk-";
export const GATEWAY_TOKEN_BODY_LEN = 48;

const ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/** `sk-` + 48 unbiased A-Za-z0-9 characters. */
export function generateGatewayApiKeyToken(): string {
  let body = "";
  while (body.length < GATEWAY_TOKEN_BODY_LEN) {
    const bytes = new Uint8Array(32);
    crypto.getRandomValues(bytes);
    for (const value of bytes) {
      if (body.length >= GATEWAY_TOKEN_BODY_LEN) {
        break;
      }
      // 248 is the largest multiple of 62 below 256.
      if (value < 248) {
        body += ALPHABET[value % 62];
      }
    }
  }
  return `${GATEWAY_TOKEN_PREFIX}${body}`;
}

export function isGatewayApiKeyToken(value: string): boolean {
  if (!value.startsWith(GATEWAY_TOKEN_PREFIX)) {
    return false;
  }
  const body = value.slice(GATEWAY_TOKEN_PREFIX.length);
  return (
    body.length === GATEWAY_TOKEN_BODY_LEN
    && /^[A-Za-z0-9]+$/.test(body)
  );
}
