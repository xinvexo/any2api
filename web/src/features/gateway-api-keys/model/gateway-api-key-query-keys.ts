export const gatewayApiKeyQueryKeys = {
  all: ["gateway-api-keys"] as const,
  list: () => ["gateway-api-keys", "list"] as const,
};
