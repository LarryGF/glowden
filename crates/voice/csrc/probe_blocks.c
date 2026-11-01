/* Locate each DTS stage's default parameters inside the ClearVoice 2 default block. */
#include "pe.h"

#include <stdio.h>
#include <string.h>

static size_t written(const uint8_t *buf, size_t cap)
{
    while (cap && buf[cap - 1] == 0xAA) cap--;
    return cap;
}

int main(int argc, char **argv)
{
    if (argc < 2) {
        fprintf(stderr, "usage: %s <clearvoice2 dll>\n", argv[0]);
        return 2;
    }
    pe_image *img = pe_load(argv[1]);
    static uint8_t cv2[4096];
    memset(cv2, 0xAA, sizeof cv2);
    int (WINAPI *cv2_defaults)(void *) = pe_export(img, "?dtsClearVoice2GetDefaultParameters@");
    cv2_defaults(cv2);
    size_t cv2_len = written(cv2, sizeof cv2);
    printf("ClearVoice2 defaults: %zu bytes\n", cv2_len);

    const char *stages[] = {"HighPassFilter", "NoiseReduction", "GatingCompressor", "MultiBandEqualiser",
                            "MultiBandCompressor", "HardLimiter", "Metering", "SampleRateConverter"};
    for (size_t s = 0; s < sizeof stages / sizeof *stages; s++) {
        char sym[96];
        snprintf(sym, sizeof sym, "?dts%sGetDefaultParameters@", stages[s]);
        int (WINAPI *defaults)(void *) = pe_export(img, sym);
        static uint8_t buf[4096];
        memset(buf, 0xAA, sizeof buf);
        int r = defaults(buf);
        size_t len = written(buf, sizeof buf);
        printf("%-20s rc=%d %3zu bytes:", stages[s], r, len);
        int found = 0;
        for (size_t off = 0; len && off + len <= cv2_len; off++) {
            if (!memcmp(cv2 + off, buf, len)) {
                printf(" at 0x%03zx", off);
                found = 1;
            }
        }
        if (!found) printf(" not found verbatim");
        printf("  [");
        for (size_t i = 0; i < len && i < 48; i += 4) {
            float f;
            memcpy(&f, buf + i, 4);
            printf(i ? " %g" : "%g", f);
        }
        printf("]\n");
    }
    return 0;
}
