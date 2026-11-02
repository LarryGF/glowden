/* Blue VO!CE as a PipeWire virtual microphone.
 *
 * Captures the real microphone, runs it through the user's own copy of the DTS ClearVoice 2
 * DLL (loaded by our PE loader) and plays the result into an Audio/Source node that apps can
 * pick as a microphone. Both streams run on the PipeWire main loop thread, the only thread
 * that calls into the DLL.
 *
 *   lhub-voice <dll> [--target NODE] [--name NAME] [--description TEXT]
 *              [f OFF VALUE]... [b OFF VALUE]... [u OFF VALUE]...
 *
 * f/b/u set a float, byte or u32 in the parameter block (offsets in hex, see docs/voice.md).
 * With --process IN.wav OUT.wav it processes a recording instead (48 kHz mono float WAV, as
 * pw-record writes it with --format f32), for comparing presets on the same take.
 */
#include "pe.h"

#include <math.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <pipewire/pipewire.h>
#include <spa/param/audio/format-utils.h>

#define RATE 48000
#define BLOCK 256
#define FIFO (RATE / 2)

typedef struct {
    uint32_t sample_rate, block_frames, channels, sample_rate2, block_frames2, two;
    uint8_t params[1024];
} cv2_config;

typedef struct {
    float buf[FIFO];
    size_t head, len;
} fifo;

static void fifo_push(fifo *f, const float *s, size_t n)
{
    for (size_t i = 0; i < n; i++) {
        if (f->len == FIFO) { /* overrun: drop the oldest sample */
            f->head = (f->head + 1) % FIFO;
            f->len--;
        }
        f->buf[(f->head + f->len++) % FIFO] = s[i];
    }
}

static size_t fifo_pop(fifo *f, float *s, size_t n)
{
    size_t got = n < f->len ? n : f->len;
    for (size_t i = 0; i < got; i++) s[i] = f->buf[(f->head + i) % FIFO];
    f->head = (f->head + got) % FIFO;
    f->len -= got;
    return got;
}

static struct {
    struct pw_main_loop *loop;
    struct pw_stream *capture, *source;
    int (WINAPI *process)(void *, float **);
    void *inst;
    float *in, *out;
    fifo raw, done;
    int primed;
    unsigned long captured, processed, failed, served;
} st;

static void on_capture(void *data)
{
    struct pw_buffer *b = pw_stream_dequeue_buffer(st.capture);
    if (!b) return;
    struct spa_data *d = &b->buffer->datas[0];
    if (d->data) {
        uint32_t n = d->chunk->size / sizeof(float);
        fifo_push(&st.raw, (float *)SPA_PTROFF(d->data, d->chunk->offset, void), n);
        st.captured += n;
    }
    pw_stream_queue_buffer(st.capture, b);
    while (st.raw.len >= BLOCK) {
        fifo_pop(&st.raw, st.in, BLOCK);
        if (st.process(st.inst, (float *[]){st.in, st.out}) == 0) {
            fifo_push(&st.done, st.out, BLOCK);
            st.processed++;
        } else {
            st.failed++;
        }
    }
}

static void on_source(void *data)
{
    struct pw_buffer *b = pw_stream_dequeue_buffer(st.source);
    if (!b) return;
    struct spa_data *d = &b->buffer->datas[0];
    if (!d->data) {
        pw_stream_queue_buffer(st.source, b);
        return;
    }
    uint32_t n = d->maxsize / sizeof(float);
    if (b->requested && b->requested < n) n = b->requested;
    float *dst = d->data;
    /* Start once a little audio is buffered, so the first reads don't underrun. */
    if (!st.primed && st.done.len >= 2 * BLOCK) st.primed = 1;
    size_t got = st.primed ? fifo_pop(&st.done, dst, n) : 0;
    if (got < n) memset(dst + got, 0, (n - got) * sizeof(float));
    st.served += got;
    d->chunk->offset = 0;
    d->chunk->stride = sizeof(float);
    d->chunk->size = n * sizeof(float);
    pw_stream_queue_buffer(st.source, b);
}

static const struct pw_stream_events capture_events = {PW_VERSION_STREAM_EVENTS, .process = on_capture};
static const struct pw_stream_events source_events = {PW_VERSION_STREAM_EVENTS, .process = on_source};

static void stop(int sig)
{
    pw_main_loop_quit(st.loop);
}

/* Quit when the service that started us is gone, instead of lingering as an orphan. */
static pid_t parent;
static void check_parent(void *data, uint64_t expirations)
{
    if (getppid() != parent) pw_main_loop_quit(st.loop);
}

static const struct spa_pod *mono_format(struct spa_pod_builder *b)
{
    return spa_format_audio_raw_build(b, SPA_PARAM_EnumFormat,
                                      &SPA_AUDIO_INFO_RAW_INIT(.format = SPA_AUDIO_FORMAT_F32, .rate = RATE,
                                                               .channels = 1, .position = {SPA_AUDIO_CHANNEL_MONO}));
}

/* Offline: run a 48 kHz mono float WAV through the engine. The output is shifted by the
 * engine's latency so it lines up with the input. */
static int process_file(void *inst, const char *in_path, const char *out_path)
{
    FILE *in = fopen(in_path, "rb");
    if (!in) {
        fprintf(stderr, "lhub-voice: cannot open %s\n", in_path);
        return 1;
    }
    uint8_t head[12];
    if (fread(head, 1, 12, in) != 12 || memcmp(head, "RIFF", 4) || memcmp(head + 8, "WAVE", 4)) {
        fprintf(stderr, "lhub-voice: %s is not a WAV file\n", in_path);
        return 1;
    }
    uint32_t len = 0, rate = 0;
    uint16_t format = 0, channels = 0;
    char id[4];
    while (fread(id, 1, 4, in) == 4 && fread(&len, 4, 1, in) == 1) {
        if (!memcmp(id, "fmt ", 4)) {
            uint8_t fmt[64] = {0};
            if (fread(fmt, 1, len < sizeof fmt ? len : sizeof fmt, in) == 0) break;
            if (len > sizeof fmt) fseek(in, len - sizeof fmt, SEEK_CUR);
            memcpy(&format, fmt, 2);
            memcpy(&channels, fmt + 2, 2);
            memcpy(&rate, fmt + 4, 4);
        } else if (!memcmp(id, "data", 4)) {
            break;
        } else {
            fseek(in, len + (len & 1), SEEK_CUR);
        }
    }
    if (memcmp(id, "data", 4) || format != 3 || channels != 1 || rate != RATE) {
        fprintf(stderr, "lhub-voice: %s must be 48 kHz mono float\n", in_path);
        return 1;
    }
    size_t n = len / sizeof(float), latency = 1280;
    float *samples = calloc(n + latency + BLOCK, sizeof(float));
    n = fread(samples, sizeof(float), n, in);
    fclose(in);
    float *result = calloc(n + latency + BLOCK, sizeof(float));
    for (size_t at = 0; at < n + latency; at += BLOCK) {
        memcpy(st.in, samples + at, BLOCK * sizeof(float));
        if (st.process(inst, (float *[]){st.in, st.out})) return 1;
        memcpy(result + at, st.out, BLOCK * sizeof(float));
    }
    FILE *out = fopen(out_path, "wb");
    if (!out) return 1;
    uint32_t bytes = n * sizeof(float), riff = 36 + bytes, fmt_len = 16, byte_rate = RATE * 4;
    uint16_t fmt_tag = 3, ch = 1, align = 4, bits = 32;
    uint32_t r = RATE;
    fwrite("RIFF", 1, 4, out);
    fwrite(&riff, 4, 1, out);
    fwrite("WAVEfmt ", 1, 8, out);
    fwrite(&fmt_len, 4, 1, out);
    fwrite(&fmt_tag, 2, 1, out);
    fwrite(&ch, 2, 1, out);
    fwrite(&r, 4, 1, out);
    fwrite(&byte_rate, 4, 1, out);
    fwrite(&align, 2, 1, out);
    fwrite(&bits, 2, 1, out);
    fwrite("data", 1, 4, out);
    fwrite(&bytes, 4, 1, out);
    fwrite(result + latency, sizeof(float), n, out);
    fclose(out);
    return 0;
}

int lhub_voice_main(int argc, char **argv)
{
    if (argc < 2) {
        fprintf(stderr, "usage: lhub-voice <dll> [--target NODE] [--name NAME] [--description TEXT] [f|b|u OFF V]...\n");
        return 2;
    }
    const char *target = NULL, *name = "lhub-bluevoice", *description = "Blue VO!CE";
    const char *process_in = NULL, *process_out = NULL;
    static cv2_config cfg = {RATE, BLOCK, 1, RATE, BLOCK, 2, {0}};

    pe_image *img = pe_load(argv[1]);
    if (!img) {
        fprintf(stderr, "lhub-voice: cannot load %s\n", argv[1]);
        return 1;
    }
    int (WINAPI *get_size)(const cv2_config *, size_t *) = pe_export(img, "?dtsClearVoice2GetInstanceSize@");
    int (WINAPI *init)(void *, const cv2_config *) = pe_export(img, "?dtsClearVoice2Init@");
    int (WINAPI *defaults)(void *) = pe_export(img, "?dtsClearVoice2GetDefaultParameters@");
    st.process = pe_export(img, "?dtsClearVoice2Process@");
    if (!get_size || !init || !defaults || !st.process) {
        fprintf(stderr, "lhub-voice: %s is not the ClearVoice 2 engine\n", argv[1]);
        return 1;
    }
    defaults(cfg.params);

    for (int i = 2; i < argc; i++) {
        if (!strcmp(argv[i], "--target") && i + 1 < argc) target = argv[++i];
        else if (!strcmp(argv[i], "--name") && i + 1 < argc) name = argv[++i];
        else if (!strcmp(argv[i], "--description") && i + 1 < argc) description = argv[++i];
        else if (!strcmp(argv[i], "--process") && i + 2 < argc) {
            process_in = argv[++i];
            process_out = argv[++i];
        }
        else if (strchr("fbu", argv[i][0]) && !argv[i][1] && i + 2 < argc) {
            long off = strtol(argv[i + 1], NULL, 16);
            if (off < 0 || off + 4 > (long)sizeof cfg.params) {
                fprintf(stderr, "lhub-voice: offset %s out of range\n", argv[i + 1]);
                return 2;
            }
            if (argv[i][0] == 'f') {
                float v = atof(argv[i + 2]);
                memcpy(cfg.params + off, &v, 4);
            } else if (argv[i][0] == 'u') {
                uint32_t v = strtoul(argv[i + 2], NULL, 10);
                memcpy(cfg.params + off, &v, 4);
            } else {
                cfg.params[off] = atoi(argv[i + 2]);
            }
            i += 2;
        } else {
            fprintf(stderr, "lhub-voice: bad argument %s\n", argv[i]);
            return 2;
        }
    }

    size_t size = 0;
    if (get_size(&cfg, &size)) {
        fprintf(stderr, "lhub-voice: engine rejected the configuration\n");
        return 1;
    }
    st.inst = aligned_alloc(64, (size + 63) & ~(size_t)63);
    memset(st.inst, 0, size);
    int r = init(st.inst, &cfg);
    if (r) {
        fprintf(stderr, "lhub-voice: engine rejected the parameters (%d)\n", r);
        return 1;
    }
    st.in = aligned_alloc(32, BLOCK * sizeof(float));
    st.out = aligned_alloc(32, BLOCK * sizeof(float));
    if (process_in) return process_file(st.inst, process_in, process_out);

    pw_init(&argc, &argv);
    st.loop = pw_main_loop_new(NULL);
    signal(SIGINT, stop);
    signal(SIGTERM, stop);
    parent = getppid();
    struct spa_source *timer = pw_loop_add_timer(pw_main_loop_get_loop(st.loop), check_parent, NULL);
    struct timespec second = {1, 0};
    pw_loop_update_timer(pw_main_loop_get_loop(st.loop), timer, &second, &second, false);

    struct pw_properties *cp = pw_properties_new(PW_KEY_MEDIA_TYPE, "Audio", PW_KEY_MEDIA_CATEGORY, "Capture",
                                                 PW_KEY_MEDIA_ROLE, "Communication", PW_KEY_NODE_NAME,
                                                 "lhub-bluevoice-input", PW_KEY_NODE_DESCRIPTION,
                                                 "Blue VO!CE input", PW_KEY_NODE_LATENCY, "256/48000", NULL);
    if (target) pw_properties_set(cp, PW_KEY_TARGET_OBJECT, target);
    /* Not offered to other apps as a stream to move: it belongs to the virtual mic. */
    pw_properties_set(cp, PW_KEY_NODE_DONT_RECONNECT, "true");
    st.capture = pw_stream_new_simple(pw_main_loop_get_loop(st.loop), "lhub-bluevoice-input", cp,
                                      &capture_events, NULL);

    struct pw_properties *sp = pw_properties_new(PW_KEY_MEDIA_TYPE, "Audio", PW_KEY_MEDIA_CLASS, "Audio/Source",
                                                 PW_KEY_NODE_NAME, name, PW_KEY_NODE_DESCRIPTION, description,
                                                 PW_KEY_NODE_LATENCY, "256/48000", NULL);
    st.source = pw_stream_new_simple(pw_main_loop_get_loop(st.loop), name, sp, &source_events, NULL);

    uint8_t buf[1024];
    struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buf, sizeof buf);
    const struct spa_pod *params[1] = {mono_format(&b)};
    pw_stream_connect(st.capture, PW_DIRECTION_INPUT, PW_ID_ANY,
                      PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_MAP_BUFFERS, params, 1);
    b = (struct spa_pod_builder)SPA_POD_BUILDER_INIT(buf, sizeof buf);
    params[0] = mono_format(&b);
    pw_stream_connect(st.source, PW_DIRECTION_OUTPUT, PW_ID_ANY, PW_STREAM_FLAG_MAP_BUFFERS, params, 1);

    fprintf(stderr, "lhub-voice: running as \"%s\"%s%s\n", description, target ? " from " : "", target ? target : "");
    pw_main_loop_run(st.loop);
    fprintf(stderr, "lhub-voice: captured %lu samples, processed %lu blocks (%lu failed), served %lu samples\n",
            st.captured, st.processed, st.failed, st.served);

    pw_stream_destroy(st.capture);
    pw_stream_destroy(st.source);
    pw_main_loop_destroy(st.loop);
    pw_deinit();
    return 0;
}
