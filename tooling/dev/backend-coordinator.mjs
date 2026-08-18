export class BackendBuildCoordinator {
  constructor({ build, deploy, onBuildFailure, debounceMs = 350 }) {
    this.build = build;
    this.deploy = deploy;
    this.onBuildFailure = onBuildFailure;
    this.debounceMs = debounceMs;
    this.desiredEpoch = 0;
    this.bindingsDirty = false;
    this.timer = undefined;
    this.loop = undefined;
    this.stopping = false;
  }

  request({ bindings = false, immediate = false } = {}) {
    if (this.stopping) return;
    this.desiredEpoch += 1;
    this.bindingsDirty ||= bindings;
    clearTimeout(this.timer);
    if (immediate) this.#startLoop();
    else this.timer = setTimeout(() => this.#startLoop(), this.debounceMs);
  }

  stop() {
    this.stopping = true;
    clearTimeout(this.timer);
  }

  async waitForIdle() {
    await this.loop;
  }

  #startLoop() {
    this.timer = undefined;
    if (this.stopping || this.loop) return;
    this.loop = this.#runLoop().finally(() => {
      this.loop = undefined;
    });
  }

  async #runLoop() {
    while (!this.stopping) {
      const epoch = this.desiredEpoch;
      const bindings = this.bindingsDirty;
      this.bindingsDirty = false;
      let artifact;
      try {
        artifact = await this.build({ bindings, epoch });
      } catch (error) {
        if (this.stopping) return;
        if (this.desiredEpoch !== epoch) continue;
        await this.onBuildFailure?.(error, epoch);
        if (this.desiredEpoch === epoch) return;
        continue;
      }
      if (this.stopping) return;
      if (this.desiredEpoch !== epoch) continue;
      try {
        await this.deploy(
          artifact,
          epoch,
          () => !this.stopping && this.desiredEpoch === epoch,
        );
      } catch (error) {
        if (this.stopping) return;
        if (this.desiredEpoch !== epoch) continue;
        await this.onBuildFailure?.(error, epoch);
        if (this.desiredEpoch === epoch) return;
        continue;
      }
      if (this.desiredEpoch === epoch) return;
    }
  }
}
