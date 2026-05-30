#include "merman.h"

#include <stdio.h>
#include <stdint.h>

int main(void) {
    static const uint8_t source[] = "flowchart TD\nA[Hello] --> B[World]";

    if (merman_abi_version() != MERMAN_ABI_VERSION) {
        fprintf(stderr, "Merman ABI mismatch\n");
        return 1;
    }

    MermanResult result = merman_render_svg(source, sizeof(source) - 1, NULL, 0);
    if (result.code != MERMAN_OK) {
        fprintf(
            stderr,
            "Merman render failed (%d): %.*s\n",
            result.code,
            (int)result.data.len,
            result.data.data == NULL ? "" : (const char*)result.data.data
        );
        merman_buffer_free(result.data);
        return 1;
    }

    printf("%.*s\n", (int)result.data.len, (const char*)result.data.data);
    merman_buffer_free(result.data);
    return 0;
}
