import {
  MermanDisposedError,
  MermanLifecycleError,
  MermanQueueSaturatedError,
  abortError,
} from "./errors.mjs";

export class BoundedExecutor {
  #active = 0;
  #concurrency;
  #disposePromise = null;
  #finishDispose = null;
  #maxQueue;
  #pending = [];
  #state = "open";

  constructor({ concurrency, maxQueue }) {
    if (!Number.isSafeInteger(concurrency) || concurrency < 1) {
      throw new RangeError("concurrency must be a positive integer.");
    }
    if (!Number.isSafeInteger(maxQueue) || maxQueue < 0) {
      throw new RangeError("maxQueue must be a non-negative integer.");
    }
    this.#concurrency = concurrency;
    this.#maxQueue = maxQueue;
  }

  get snapshot() {
    return Object.freeze({
      active: this.#active,
      pending: this.#pending.length,
      concurrency: this.#concurrency,
      maxQueue: this.#maxQueue,
      state: this.#state,
    });
  }

  submit(run, { signal } = {}) {
    if (this.#state !== "open") return Promise.reject(new MermanDisposedError());
    if (signal?.aborted) return Promise.reject(abortError());

    if (this.#active < this.#concurrency) {
      return this.#start({ run, signal });
    }
    if (this.#pending.length >= this.#maxQueue) {
      return Promise.reject(new MermanQueueSaturatedError(this.#maxQueue));
    }

    return new Promise((resolve, reject) => {
      const job = { abortListener: null, reject, resolve, run, signal, started: false };
      if (signal) {
        job.abortListener = () => {
          if (job.started) return;
          const index = this.#pending.indexOf(job);
          if (index === -1) return;
          this.#pending.splice(index, 1);
          reject(abortError());
        };
        signal.addEventListener("abort", job.abortListener, { once: true });
      }
      this.#pending.push(job);
      // Close the race between the initial admission check and listener registration. Calling the
      // idempotent listener is safe even when an abort event already removed the job.
      if (signal?.aborted) job.abortListener?.();
    });
  }

  assertSyncAvailable() {
    this.assertOpen();
    if (this.#active !== 0 || this.#pending.length !== 0) {
      throw new MermanLifecycleError(
        "renderSvgSync() cannot run while asynchronous operations are active or queued.",
      );
    }
  }

  assertOpen() {
    if (this.#state !== "open") throw new MermanDisposedError();
  }

  dispose() {
    if (this.#disposePromise) return this.#disposePromise;
    this.#state = "disposing";
    const disposed = new MermanDisposedError();
    for (const job of this.#pending.splice(0)) {
      this.#removeAbortListener(job);
      job.reject(disposed);
    }
    this.#disposePromise = new Promise((resolve) => {
      this.#finishDispose = resolve;
      this.#completeDisposeIfIdle();
    });
    return this.#disposePromise;
  }

  #start(job) {
    job.started = true;
    this.#removeAbortListener(job);
    this.#active += 1;
    const operation = Promise.resolve().then(job.run);
    const observed = job.resolve || job.reject
      ? operation.then(job.resolve, job.reject)
      : operation;
    return observed.finally(() => {
      this.#active -= 1;
      this.#pump();
      this.#completeDisposeIfIdle();
    });
  }

  #pump() {
    while (
      this.#state === "open" &&
      this.#active < this.#concurrency &&
      this.#pending.length > 0
    ) {
      const job = this.#pending.shift();
      void this.#start(job);
    }
  }

  #completeDisposeIfIdle() {
    if (this.#state !== "disposing" || this.#active !== 0) return;
    this.#state = "disposed";
    this.#finishDispose?.();
    this.#finishDispose = null;
  }

  #removeAbortListener(job) {
    if (job.signal && job.abortListener) {
      job.signal.removeEventListener("abort", job.abortListener);
      job.abortListener = null;
    }
  }
}
