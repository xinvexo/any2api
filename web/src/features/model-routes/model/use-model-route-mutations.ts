import { useMutation, useQueryClient } from "@tanstack/react-query";

import type {
  ModelRouteConfiguration,
  ModelRouteWriteInput,
} from "../api/model-route-contracts";
import {
  createModelRoute,
  deleteModelRoute,
  updateModelRoute,
} from "../api/model-route-api";
import { selectNewestModelRouteConfiguration } from "./model-route-cache";
import { modelRouteQueryKeys } from "./model-route-query-keys";

export function useModelRouteMutations() {
  const queryClient = useQueryClient();
  const publish = (configuration: ModelRouteConfiguration) => {
    queryClient.setQueryData<ModelRouteConfiguration>(
      modelRouteQueryKeys.list(),
      (current) => selectNewestModelRouteConfiguration(current, configuration),
    );
    void queryClient.invalidateQueries({ queryKey: modelRouteQueryKeys.all });
  };
  const refreshAfterFailure = async () => {
    await queryClient.refetchQueries({ queryKey: modelRouteQueryKeys.all, type: "active" });
  };

  const create = useMutation({
    mutationFn: createModelRoute,
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: ModelRouteWriteInput }) =>
      updateModelRoute(id, input),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  const remove = useMutation({
    mutationFn: ({
      id,
      expectedRevision,
      expectedConfigVersion,
    }: {
      id: string;
      expectedRevision: number;
      expectedConfigVersion: number;
    }) => deleteModelRoute(id, expectedRevision, expectedConfigVersion),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });

  return {
    create,
    update,
    remove,
    isPending: create.isPending || update.isPending || remove.isPending,
  };
}
