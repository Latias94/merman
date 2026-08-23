package io.merman

import java.util.concurrent.atomic.AtomicLong

/**
 * Shared cooperative cancellation and relative-deadline control for one or more operations.
 *
 * [timeoutMs] installs a monotonic deadline relative to construction. Retain the same object
 * on another thread and call [cancel] while synchronous execution is in progress. Cancellation is
 * cooperative, so an opaque host callback may finish before the next native checkpoint.
 *
 * Call [close] or [release] when the control is no longer needed. Release is idempotent and does
 * not invalidate a control clone already acquired by an in-flight native operation.
 */
class MermanOperationControl @JvmOverloads constructor(
    timeoutMs: Long? = null,
) : AutoCloseable {
    private val token = AtomicLong(
        nativeNew(timeoutMs ?: 0L, timeoutMs != null),
    )

    /** Requests cooperative cancellation and is safe to call from another thread. */
    fun cancel() {
        nativeCancel(token.get())
    }

    /** Reports whether explicit cancellation was requested on this shared control. */
    fun isCancelled(): Boolean = nativeIsCancelled(token.get())

    /** Releases the native registry token. Repeated calls are no-ops. */
    fun release() {
        val current = token.getAndSet(0L)
        if (current != 0L) {
            nativeRelease(current)
        }
    }

    override fun close() = release()

    internal fun tokenForExecution(): Long = token.get()

    private companion object {
        init {
            Merman.ensureNativeReady()
        }

        @JvmStatic
        private external fun nativeNew(timeoutMs: Long, hasTimeoutMs: Boolean): Long

        @JvmStatic
        private external fun nativeCancel(token: Long)

        @JvmStatic
        private external fun nativeIsCancelled(token: Long): Boolean

        @JvmStatic
        private external fun nativeRelease(token: Long)
    }
}
