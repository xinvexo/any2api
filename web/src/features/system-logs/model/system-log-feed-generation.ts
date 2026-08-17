let generation = 0;

export function currentSystemLogFeedGeneration() {
  return generation;
}

export function advanceSystemLogFeedGeneration() {
  generation += 1;
}
