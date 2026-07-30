import { useMutation, useQuery } from "@tanstack/react-query";

import {
  checkApplicationUpdate,
  getApplicationAbout,
} from "../api/update-api";

const applicationUpdateKey = ["application-update"] as const;

export function useApplicationUpdate() {
  const about = useQuery({
    queryKey: [...applicationUpdateKey, "about"],
    queryFn: ({ signal }) => getApplicationAbout(signal),
  });
  const check = useMutation({ mutationFn: checkApplicationUpdate, retry: false });
  return {
    about,
    check,
    isPending: check.isPending,
  };
}
