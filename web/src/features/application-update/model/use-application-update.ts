import { useMutation, useQuery } from "@tanstack/react-query";

import {
  checkApplicationUpdate,
  getApplicationAbout,
  installApplicationUpdate,
} from "../api/update-api";

const applicationUpdateKey = ["application-update"] as const;

export function useApplicationUpdate() {
  const about = useQuery({
    queryKey: [...applicationUpdateKey, "about"],
    queryFn: ({ signal }) => getApplicationAbout(signal),
  });
  const check = useMutation({ mutationFn: checkApplicationUpdate, retry: false });
  const install = useMutation({ mutationFn: installApplicationUpdate, retry: false });

  return {
    about,
    check,
    install,
    isPending: check.isPending || install.isPending,
  };
}
