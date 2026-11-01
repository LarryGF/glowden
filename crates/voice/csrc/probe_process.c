/* Create a ClearVoice 2 instance with default parameters and push a test tone through it. */
#include "pe.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    uint32_t sample_rate;   /* 22050..192000 */
    uint32_t block_frames;  /* 32..8092 */
    uint32_t channels;      /* 1..2 */
    uint32_t sample_rate2;  /* must equal sample_rate */
    uint32_t block_frames2; /* must equal block_frames */
    uint32_t two;           /* must be 2 */
    uint8_t params[1024];   /* parameter block, filled by GetDefaultParameters */
} cv2_config;

int main(int argc, char **argv)
{
    if (argc < 2) {
        fprintf(stderr, "usage: %s <clearvoice2 dll> [block_frames]\n", argv[0]);
        return 2;
    }
    pe_image *img = pe_load(argv[1]);
    int (WINAPI *get_size)(const cv2_config *, size_t *) = pe_export(img, "?dtsClearVoice2GetInstanceSize@");
    int (WINAPI *init)(void *, const cv2_config *) = pe_export(img, "?dtsClearVoice2Init@");
    int (WINAPI *defaults)(void *) = pe_export(img, "?dtsClearVoice2GetDefaultParameters@");
    int (WINAPI *update)(void *, void *) = pe_export(img, "?dtsClearVoice2UpdateParameters@");
    int (WINAPI *process)(void *, float **) = pe_export(img, "?dtsClearVoice2Process@");
    int (WINAPI *latency)(void *, int *) = pe_export(img, "?dtsClearVoice2GetCurrentLatency@");

    unsigned frames = argc > 2 ? atoi(argv[2]) : 256;
    static cv2_config cfg = {48000, 0, 1, 48000, 0, 2, {0}};
    cfg.block_frames = cfg.block_frames2 = frames;
    defaults(cfg.params);
    size_t size = 0;
    int r = get_size(&cfg, &size);
    printf("GetInstanceSize(rate=%u frames=%u ch=%u) -> %d, size=%zu\n", cfg.sample_rate, cfg.block_frames, cfg.channels, r, size);
    if (r) return 1;

    void *inst = aligned_alloc(64, (size + 63) & ~63UL);
    memset(inst, 0, size);
    r = init(inst, &cfg);
    printf("Init -> %d\n", r);
    if (r) return 1;

    printf("UpdateParameters(defaults) -> %d\n", update(inst, cfg.params));
    int lat = -1;
    latency(inst, &lat);
    printf("latency: %d frames\n", lat);

    /* Process takes {input, output}, both 32-byte aligned. */
    float *buf = aligned_alloc(32, cfg.block_frames * sizeof(float));
    float *out = aligned_alloc(32, cfg.block_frames * sizeof(float));
    float *chans[2] = {buf, out};
    double in_rms = 0, out_rms = 0;
    unsigned blocks = 48000 / cfg.block_frames * 2;
    for (unsigned b = 0; b < blocks; b++) {
        for (unsigned i = 0; i < cfg.block_frames; i++) {
            double t = (double)(b * cfg.block_frames + i) / 48000;
            buf[i] = 0.1f * sinf(2 * M_PI * 1000 * t);
            if (b >= blocks / 2) in_rms += buf[i] * buf[i];
        }
        memset(out, 0, cfg.block_frames * sizeof(float));
        r = process(inst, chans);
        if (r) { printf("Process -> %d at block %u\n", r, b); return 1; }
        if (b >= blocks / 2)
            for (unsigned i = 0; i < cfg.block_frames; i++) out_rms += out[i] * out[i];
    }
    unsigned n = (blocks - blocks / 2) * cfg.block_frames;
    in_rms = sqrt(in_rms / n);
    out_rms = sqrt(out_rms / n);
    printf("1 kHz tone, last second: in %.1f dBFS, out %.1f dBFS\n", 20 * log10(in_rms), 20 * log10(out_rms + 1e-12));
    return 0;
}
