export const routeInspectionQueryKeys = {
  all: ["route-inspection"] as const,
  current: () => [...routeInspectionQueryKeys.all, "current"] as const,
};
