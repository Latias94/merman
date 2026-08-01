export interface OperationQueue {
  enqueue<T>(operation: () => T | Promise<T>): Promise<T>;
}

export function createOperationQueue(): OperationQueue {
  let tail: Promise<void> = Promise.resolve();
  return {
    enqueue<T>(operation: () => T | Promise<T>): Promise<T> {
      const result = tail.then(operation);
      tail = result.then(
        () => undefined,
        () => undefined
      );
      return result;
    },
  };
}
