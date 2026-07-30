type Listener = EventListenerOrEventListenerObject;

export class FakeEventSource {
  static readonly instances: FakeEventSource[] = [];

  readonly url: string;
  closed = false;
  private readonly listeners = new Map<string, Set<Listener>>();

  constructor(url: string | URL) {
    this.url = String(url);
    FakeEventSource.instances.push(this);
  }

  addEventListener(type: string, listener: Listener | null) {
    if (!listener) {
      return;
    }
    const listeners = this.listeners.get(type) ?? new Set<Listener>();
    listeners.add(listener);
    this.listeners.set(type, listeners);
  }

  removeEventListener(type: string, listener: Listener | null) {
    if (listener) {
      this.listeners.get(type)?.delete(listener);
    }
  }

  emit(type: string, data = "1") {
    const event = new MessageEvent(type, { data });
    for (const listener of this.listeners.get(type) ?? []) {
      if (typeof listener === "function") {
        listener.call(this, event);
      } else {
        listener.handleEvent(event);
      }
    }
  }

  close() {
    this.closed = true;
  }

  static reset() {
    FakeEventSource.instances.length = 0;
  }
}
