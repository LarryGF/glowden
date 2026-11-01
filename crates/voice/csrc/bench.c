/* Real-time factor of ClearVoice 2 at 48 kHz mono, plus gate check on near-silence. */
#include "pe.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef struct { uint32_t rate, frames, ch, rate2, frames2, two; uint8_t params[1024]; } cfg_t;

static double rms_db(const float *x, unsigned n) { double s = 0; for (unsigned i = 0; i < n; i++) s += x[i] * x[i]; return 10 * log10(s / n + 1e-20); }

int main(int argc, char **argv)
{
    pe_image *img = pe_load(argv[1]);
    int (WINAPI *get_size)(void *, size_t *) = pe_export(img, "?dtsClearVoice2GetInstanceSize@");
    int (WINAPI *init)(void *, void *) = pe_export(img, "?dtsClearVoice2Init@");
    int (WINAPI *defaults)(void *) = pe_export(img, "?dtsClearVoice2GetDefaultParameters@");
    int (WINAPI *process)(void *, float **) = pe_export(img, "?dtsClearVoice2Process@");
    static cfg_t c = {48000, 256, 1, 48000, 256, 2, {0}};
    defaults(c.params);
    size_t sz; get_size(&c, &sz);
    void *inst = aligned_alloc(64, (sz + 63) & ~63UL); memset(inst, 0, sz); init(inst, &c);
    float *in = aligned_alloc(32, 1024), *out = aligned_alloc(32, 1024), *io[2] = {in, out};
    srand(1);
    const unsigned blocks = 48000 / 256 * 10;
    double quiet_in = 0, quiet_out = 0;
    struct timespec a, b; clock_gettime(CLOCK_MONOTONIC, &a);
    for (unsigned k = 0; k < blocks; k++) {
        int quiet = (k / 188) % 2;   /* alternate 1 s of speech-level noise, 1 s of near-silence */
        float level = quiet ? 0.0005f : 0.1f;
        for (int i = 0; i < 256; i++) in[i] = level * ((float)rand() / RAND_MAX * 2 - 1);
        int measure = quiet && k % 188 > 94;
        if (measure) quiet_in += rms_db(in, 256);
        process(inst, io);
        if (measure) quiet_out += rms_db(out, 256);
    }
    clock_gettime(CLOCK_MONOTONIC, &b);
    double secs = b.tv_sec - a.tv_sec + (b.tv_nsec - a.tv_nsec) / 1e9;
    printf("10 s of audio processed in %.3f s -> %.1fx faster than real time (%.1f%% of one core)\n", secs, 10 / secs, secs * 10);
    unsigned qn = 0; for (unsigned k = 0; k < blocks; k++) if ((k / 188) % 2 && k % 188 > 94) qn++;
    printf("near-silence: in %.1f dBFS, out %.1f dBFS\n", quiet_in / qn, quiet_out / qn);
    return 0;
}
