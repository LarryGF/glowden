/* Load the ClearVoice 2 DLL and dump its default parameter block. */
#include "pe.h"

#include <stdio.h>
#include <string.h>

int main(int argc, char **argv)
{
    if (argc < 2) {
        fprintf(stderr, "usage: %s logi_audio_dts_clearvoice_2_capture_apo.dll\n", argv[0]);
        return 2;
    }
    pe_image *img = pe_load(argv[1]);
    printf("loaded at %p\n", (void *)img->base);

    int (WINAPI *defaults)(void *) = pe_export(img, "?dtsClearVoice2GetDefaultParameters@");
    static uint8_t buf[8192];
    memset(buf, 0xAA, sizeof buf);
    int r = defaults(buf);
    size_t used = sizeof buf;
    while (used && buf[used - 1] == 0xAA) used--;
    printf("GetDefaultParameters -> %d, %zu bytes written\n", r, used);
    for (size_t i = 0; i < used; i += 4) {
        uint32_t u;
        float f;
        memcpy(&u, buf + i, 4);
        memcpy(&f, buf + i, 4);
        printf("%04zx: %08x  i=%-11d f=%g\n", i, u, (int)u, f);
    }
    return 0;
}
