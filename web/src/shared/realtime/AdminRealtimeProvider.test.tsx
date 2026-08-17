import { act, render, screen } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, expect, test, vi } from "vitest";

import { FakeEventSource } from "@/test/fake-event-source";

import { AdminRealtimeProvider } from "./AdminRealtimeProvider";
import { useAdminEvent, useAdminRealtimeStatus } from "./use-admin-event";

afterEach(() => {
  FakeEventSource.reset();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("shares one authenticated EventSource and dispatches decoded payloads", () => {
  vi.stubGlobal("EventSource", FakeEventSource);
  const overview = vi.fn();
  const logs = vi.fn();

  render(
    <AdminRealtimeProvider authenticated>
      <EventConsumer eventName="overview_snapshot" onEvent={overview} />
      <EventConsumer eventName="request_logs_changed" onEvent={logs} />
      <StatusConsumer />
    </AdminRealtimeProvider>,
  );

  expect(FakeEventSource.instances).toHaveLength(1);
  expect(FakeEventSource.instances[0]?.url).toBe("/api/admin/events");
  expect(screen.getByText("disconnected stale")).toBeInTheDocument();

  act(() => FakeEventSource.instances[0]?.emit("open"));
  expect(screen.getByText("connected stale")).toBeInTheDocument();

  act(() => {
    FakeEventSource.instances[0]?.emit(
      "overview_snapshot",
      '{"sampled_at_ms":42,"freshness":"fresh"}',
    );
    FakeEventSource.instances[0]?.emit("request_logs_changed", "7");
  });
  expect(overview).toHaveBeenCalledWith({ sampled_at_ms: 42, freshness: "fresh" });
  expect(logs).toHaveBeenCalledWith(7);
  expect(screen.getByText("connected fresh")).toBeInTheDocument();
});

test("keeps only one live source in StrictMode and closes it when authentication ends", () => {
  vi.stubGlobal("EventSource", FakeEventSource);

  const view = render(
    <StrictMode>
      <AdminRealtimeProvider authenticated>
        <StatusConsumer />
      </AdminRealtimeProvider>
    </StrictMode>,
  );

  expect(FakeEventSource.instances.filter((source) => !source.closed)).toHaveLength(1);
  const liveSource = FakeEventSource.instances.find((source) => !source.closed);

  view.rerender(
    <StrictMode>
      <AdminRealtimeProvider authenticated={false}>
        <StatusConsumer />
      </AdminRealtimeProvider>
    </StrictMode>,
  );

  expect(liveSource?.closed).toBe(true);
  expect(FakeEventSource.instances.filter((source) => !source.closed)).toHaveLength(0);
  expect(screen.getByText("disconnected stale")).toBeInTheDocument();
});

test("marks a connected snapshot stale when fresh samples stop arriving", () => {
  vi.useFakeTimers();
  vi.stubGlobal("EventSource", FakeEventSource);

  render(
    <AdminRealtimeProvider authenticated>
      <EventConsumer eventName="overview_snapshot" onEvent={vi.fn()} />
      <StatusConsumer />
    </AdminRealtimeProvider>,
  );

  act(() => {
    FakeEventSource.instances[0]?.emit("open");
    FakeEventSource.instances[0]?.emit("overview_snapshot", '{"freshness":"fresh"}');
  });
  expect(screen.getByText("connected fresh")).toBeInTheDocument();

  act(() => vi.advanceTimersByTime(7_000));
  expect(screen.getByText("connected stale")).toBeInTheDocument();
});

test("bounds reconnect attempts and checks authentication once per failure burst", async () => {
  vi.useFakeTimers();
  vi.stubGlobal("EventSource", FakeEventSource);
  const refresh = vi.fn(async () => undefined);

  render(
    <AdminRealtimeProvider authenticated onAuthRefresh={refresh}>
      <StatusConsumer />
    </AdminRealtimeProvider>,
  );

  await failAndAdvance(1_000);
  await failAndAdvance(2_000);
  await failAndAdvance(5_000);
  act(() => FakeEventSource.instances.at(-1)?.emit("error"));
  await act(async () => undefined);
  expect(refresh).toHaveBeenCalledTimes(1);

  await act(async () => vi.advanceTimersByTimeAsync(5_000));
  await failAndAdvance(1_000);
  await failAndAdvance(2_000);
  await failAndAdvance(5_000);
  act(() => FakeEventSource.instances.at(-1)?.emit("error"));
  await act(async () => vi.advanceTimersByTimeAsync(60_000));

  expect(refresh).toHaveBeenCalledTimes(1);
  expect(FakeEventSource.instances).toHaveLength(8);
  expect(FakeEventSource.instances.filter((source) => !source.closed)).toHaveLength(0);
});

function EventConsumer({
  eventName,
  onEvent,
}: {
  eventName: string;
  onEvent: (payload: unknown) => void;
}) {
  useAdminEvent(eventName, true, onEvent);
  return null;
}

function StatusConsumer() {
  const status = useAdminRealtimeStatus();
  return <p>{`${status.connected ? "connected" : "disconnected"} ${status.stale ? "stale" : "fresh"}`}</p>;
}

async function failAndAdvance(delay: number) {
  act(() => FakeEventSource.instances.at(-1)?.emit("error"));
  await act(async () => vi.advanceTimersByTimeAsync(delay));
}
