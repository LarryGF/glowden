/* Screen sampler: asks the desktop's ScreenCast portal for a monitor (the first time the
 * desktop shows its picker; after that a restore token skips it), receives the screen over
 * PipeWire and writes the average color of each cell of a grid to stdout, about 30 times a
 * second, as COLS*ROWS RGB byte triples, row by row from the top left.
 *
 *   lhub-sampler [--grid COLSxROWS] [--token FILE]
 *
 * FILE keeps the portal's restore token between runs. Quits when stdout closes or the
 * process that started it goes away.
 */
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <spa/param/param.h>
#include <systemd/sd-bus.h>

#define PORTAL "org.freedesktop.portal.Desktop"
#define PORTAL_PATH "/org/freedesktop/portal/desktop"
#define SCREENCAST "org.freedesktop.portal.ScreenCast"

static int cols = 16, rows = 9;
static const char *token_file;

/* ---- portal ---- */

static sd_bus *bus;
static char sender[128];
static int counter;

typedef struct {
    int done;
    uint32_t response;
    sd_bus_message *results;
} reply;

static int on_response(sd_bus_message *m, void *data, sd_bus_error *err)
{
    reply *r = data;
    if (sd_bus_message_read(m, "u", &r->response) < 0) r->response = 99;
    r->results = sd_bus_message_ref(m);
    r->done = 1;
    return 0;
}

/* Calls a portal method whose answer comes as a Response signal on a request object, and
 * waits for it. `fill` appends the method's arguments, ending with the options dict open
 * so the handle token can be added. */
typedef int (*filler)(sd_bus_message *m, void *data);

static int portal_request(const char *method, filler fill, void *data, reply *r)
{
    char token[32], path[256];
    snprintf(token, sizeof token, "lhub%d", ++counter);
    snprintf(path, sizeof path, PORTAL_PATH "/request/%s/%s", sender, token);
    sd_bus_slot *slot = NULL;
    memset(r, 0, sizeof *r);
    int e = sd_bus_match_signal(bus, &slot, PORTAL, path, "org.freedesktop.portal.Request", "Response",
                                on_response, r);
    if (e < 0) return e;
    sd_bus_message *m = NULL;
    e = sd_bus_message_new_method_call(bus, &m, PORTAL, PORTAL_PATH, SCREENCAST, method);
    if (e >= 0) e = fill(m, data);
    if (e >= 0) e = sd_bus_message_append(m, "{sv}", "handle_token", "s", token);
    if (e >= 0) e = sd_bus_message_close_container(m);
    sd_bus_error err = SD_BUS_ERROR_NULL;
    if (e >= 0) e = sd_bus_call(bus, m, 0, &err, NULL);
    sd_bus_message_unref(m);
    if (e < 0) {
        fprintf(stderr, "lhub-sampler: %s: %s\n", method, err.message ? err.message : strerror(-e));
        sd_bus_error_free(&err);
        sd_bus_slot_unref(slot);
        return e;
    }
    while (!r->done) {
        e = sd_bus_process(bus, NULL);
        if (e < 0) break;
        if (e == 0 && sd_bus_wait(bus, UINT64_MAX) < 0) break;
    }
    sd_bus_slot_unref(slot);
    if (!r->done) return -EIO;
    if (r->response != 0) {
        fprintf(stderr, "lhub-sampler: %s was %s\n", method, r->response == 1 ? "cancelled" : "refused");
        return -ECANCELED;
    }
    return 0;
}

/* Finds `key` in the a{sv} results of a Response and enters its variant of `type`. */
static int find_result(sd_bus_message *m, const char *key, const char *type)
{
    sd_bus_message_rewind(m, 1);
    uint32_t response;
    sd_bus_message_read(m, "u", &response);
    if (sd_bus_message_enter_container(m, 'a', "{sv}") <= 0) return -ENOENT;
    while (sd_bus_message_enter_container(m, 'e', "sv") > 0) {
        const char *k;
        sd_bus_message_read(m, "s", &k);
        if (!strcmp(k, key)) return sd_bus_message_enter_container(m, 'v', type);
        sd_bus_message_skip(m, "v");
        sd_bus_message_exit_container(m);
    }
    return -ENOENT;
}

static char session[256];

static int fill_create(sd_bus_message *m, void *data)
{
    int e = sd_bus_message_open_container(m, 'a', "{sv}");
    if (e >= 0) e = sd_bus_message_append(m, "{sv}", "session_handle_token", "s", "lhubsampler");
    return e;
}

static int fill_select(sd_bus_message *m, void *data)
{
    const char *restore = data;
    int e = sd_bus_message_append(m, "o", session);
    if (e >= 0) e = sd_bus_message_open_container(m, 'a', "{sv}");
    if (e >= 0) e = sd_bus_message_append(m, "{sv}", "types", "u", 1u); /* monitors */
    if (e >= 0) e = sd_bus_message_append(m, "{sv}", "multiple", "b", 0);
    if (e >= 0) e = sd_bus_message_append(m, "{sv}", "cursor_mode", "u", 1u); /* hidden */
    if (e >= 0) e = sd_bus_message_append(m, "{sv}", "persist_mode", "u", 2u); /* until revoked */
    if (e >= 0 && restore && *restore) e = sd_bus_message_append(m, "{sv}", "restore_token", "s", restore);
    return e;
}

static int fill_start(sd_bus_message *m, void *data)
{
    int e = sd_bus_message_append(m, "os", session, "");
    if (e >= 0) e = sd_bus_message_open_container(m, 'a', "{sv}");
    return e;
}

/* Returns the PipeWire fd and the stream's node id, or -1. */
static int open_screencast(uint32_t *node)
{
    if (sd_bus_open_user(&bus) < 0) {
        fprintf(stderr, "lhub-sampler: no session bus\n");
        return -1;
    }
    const char *unique;
    sd_bus_get_unique_name(bus, &unique);
    size_t j = 0;
    for (const char *c = unique + 1; *c && j < sizeof sender - 1; c++) sender[j++] = *c == '.' ? '_' : *c;
    sender[j] = 0;

    reply r;
    if (portal_request("CreateSession", fill_create, NULL, &r) < 0) return -1;
    const char *handle = NULL;
    if (find_result(r.results, "session_handle", "s") >= 0) sd_bus_message_read(r.results, "s", &handle);
    if (!handle) {
        fprintf(stderr, "lhub-sampler: the portal gave no session\n");
        return -1;
    }
    snprintf(session, sizeof session, "%s", handle);
    sd_bus_message_unref(r.results);

    char restore[512] = "";
    FILE *f = token_file ? fopen(token_file, "r") : NULL;
    if (f) {
        if (!fgets(restore, sizeof restore, f)) restore[0] = 0;
        restore[strcspn(restore, "\n")] = 0;
        fclose(f);
    }
    if (portal_request("SelectSources", fill_select, restore, &r) < 0) return -1;
    sd_bus_message_unref(r.results);
    if (portal_request("Start", fill_start, NULL, &r) < 0) return -1;

    const char *new_token = NULL;
    if (find_result(r.results, "restore_token", "s") >= 0) sd_bus_message_read(r.results, "s", &new_token);
    if (new_token && token_file && (f = fopen(token_file, "w"))) {
        fprintf(f, "%s\n", new_token);
        fclose(f);
    }
    *node = 0;
    if (find_result(r.results, "streams", "a(ua{sv})") >= 0 && sd_bus_message_enter_container(r.results, 'a', "(ua{sv})") > 0 &&
        sd_bus_message_enter_container(r.results, 'r', "ua{sv}") > 0)
        sd_bus_message_read(r.results, "u", node);
    sd_bus_message_unref(r.results);
    if (!*node) {
        fprintf(stderr, "lhub-sampler: the portal gave no stream\n");
        return -1;
    }

    sd_bus_message *m = NULL, *answer = NULL;
    sd_bus_error err = SD_BUS_ERROR_NULL;
    int e = sd_bus_message_new_method_call(bus, &m, PORTAL, PORTAL_PATH, SCREENCAST, "OpenPipeWireRemote");
    if (e >= 0) e = sd_bus_message_append(m, "oa{sv}", session, 0);
    if (e >= 0) e = sd_bus_call(bus, m, 0, &err, &answer);
    sd_bus_message_unref(m);
    int fd = -1;
    if (e >= 0 && sd_bus_message_read(answer, "h", &fd) >= 0) fd = fcntl(fd, F_DUPFD_CLOEXEC, 3);
    else fprintf(stderr, "lhub-sampler: OpenPipeWireRemote: %s\n", err.message ? err.message : strerror(-e));
    sd_bus_error_free(&err);
    sd_bus_message_unref(answer);
    return fd;
}

/* ---- video ---- */

static struct {
    struct pw_main_loop *loop;
    struct pw_stream *stream;
    struct spa_hook listener;
    struct spa_video_info_raw format;
    int have_format;
    struct timespec last;
} st;

static void on_param_changed(void *data, uint32_t id, const struct spa_pod *param)
{
    if (id != SPA_PARAM_Format || !param) return;
    if (spa_format_video_raw_parse(param, &st.format) < 0) return;
    st.have_format = 1;
    fprintf(stderr, "lhub-sampler: %ux%u\n", st.format.size.width, st.format.size.height);
    /* Plain memory buffers, which this process can read directly. */
    uint8_t buf[256];
    struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buf, sizeof buf);
    const struct spa_pod *params[1] = {spa_pod_builder_add_object(
        &b, SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers, SPA_PARAM_BUFFERS_dataType,
        SPA_POD_CHOICE_FLAGS_Int((1 << SPA_DATA_MemPtr) | (1 << SPA_DATA_MemFd)))};
    pw_stream_update_params(st.stream, params, 1);
}

static void on_process(void *data)
{
    struct pw_buffer *b = pw_stream_dequeue_buffer(st.stream);
    if (!b) return;
    struct spa_data *d = &b->buffer->datas[0];
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    double since = (now.tv_sec - st.last.tv_sec) + (now.tv_nsec - st.last.tv_nsec) / 1e9;
    if (st.have_format && d->data && d->chunk->size && since >= 1.0 / 30) {
        st.last = now;
        uint32_t w = st.format.size.width, h = st.format.size.height;
        int stride = d->chunk->stride ? d->chunk->stride : (int)w * 4;
        const uint8_t *px = SPA_PTROFF(d->data, d->chunk->offset, const uint8_t);
        int rgb = st.format.format == SPA_VIDEO_FORMAT_RGBx || st.format.format == SPA_VIDEO_FORMAT_RGBA;
        static uint8_t out[64 * 64 * 3];
        for (int r = 0; r < rows; r++)
            for (int c = 0; c < cols; c++) {
                uint32_t x0 = w * c / cols, x1 = w * (c + 1) / cols, y0 = h * r / rows, y1 = h * (r + 1) / rows;
                uint64_t sum[3] = {0}, n = 0;
                for (uint32_t y = y0; y < y1; y += 4)
                    for (uint32_t x = x0; x < x1; x += 4) {
                        const uint8_t *p = px + (size_t)y * stride + x * 4;
                        sum[0] += p[rgb ? 0 : 2];
                        sum[1] += p[1];
                        sum[2] += p[rgb ? 2 : 0];
                        n++;
                    }
                uint8_t *o = out + 3 * (r * cols + c);
                for (int k = 0; k < 3; k++) o[k] = n ? sum[k] / n : 0;
            }
        if (fwrite(out, 3, cols * rows, stdout) != (size_t)(cols * rows) || fflush(stdout)) pw_main_loop_quit(st.loop);
    }
    pw_stream_queue_buffer(st.stream, b);
}

static const struct pw_stream_events stream_events = {
    PW_VERSION_STREAM_EVENTS, .param_changed = on_param_changed, .process = on_process};

static pid_t parent;
static void check_parent(void *data, uint64_t expirations)
{
    if (getppid() != parent) pw_main_loop_quit(st.loop);
}

static void stop(int sig)
{
    pw_main_loop_quit(st.loop);
}

int lhub_sampler_main(int argc, char **argv)
{
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--grid") && i + 1 < argc) {
            if (sscanf(argv[++i], "%dx%d", &cols, &rows) != 2 || cols < 1 || rows < 1 || cols > 64 || rows > 64) {
                fprintf(stderr, "lhub-sampler: bad grid %s\n", argv[i]);
                return 2;
            }
        } else if (!strcmp(argv[i], "--token") && i + 1 < argc) {
            token_file = argv[++i];
        } else {
            fprintf(stderr, "usage: lhub-sampler [--grid COLSxROWS] [--token FILE]\n");
            return 2;
        }
    }
    signal(SIGPIPE, SIG_IGN);
    parent = getppid();
    uint32_t node;
    int fd = open_screencast(&node);
    if (fd < 0) return 1;

    pw_init(&argc, &argv);
    st.loop = pw_main_loop_new(NULL);
    signal(SIGINT, stop);
    signal(SIGTERM, stop);
    struct pw_context *context = pw_context_new(pw_main_loop_get_loop(st.loop), NULL, 0);
    struct pw_core *core = pw_context_connect_fd(context, fd, NULL, 0);
    if (!core) {
        fprintf(stderr, "lhub-sampler: cannot connect to the screen stream\n");
        return 1;
    }
    st.stream = pw_stream_new(core, "lhub-sampler",
                              pw_properties_new(PW_KEY_MEDIA_TYPE, "Video", PW_KEY_MEDIA_CATEGORY, "Capture",
                                                PW_KEY_MEDIA_ROLE, "Screen", NULL));
    pw_stream_add_listener(st.stream, &st.listener, &stream_events, NULL);

    uint8_t buf[1024];
    struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buf, sizeof buf);
    const struct spa_pod *params[1] = {spa_pod_builder_add_object(
        &b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat, SPA_FORMAT_mediaType, SPA_POD_Id(SPA_MEDIA_TYPE_video),
        SPA_FORMAT_mediaSubtype, SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw), SPA_FORMAT_VIDEO_format,
        SPA_POD_CHOICE_ENUM_Id(5, SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_BGRA,
                               SPA_VIDEO_FORMAT_RGBx, SPA_VIDEO_FORMAT_RGBA),
        SPA_FORMAT_VIDEO_size,
        SPA_POD_CHOICE_RANGE_Rectangle(&SPA_RECTANGLE(640, 360), &SPA_RECTANGLE(1, 1), &SPA_RECTANGLE(8192, 8192)),
        SPA_FORMAT_VIDEO_framerate,
        SPA_POD_CHOICE_RANGE_Fraction(&SPA_FRACTION(30, 1), &SPA_FRACTION(0, 1), &SPA_FRACTION(240, 1)))};
    pw_stream_connect(st.stream, PW_DIRECTION_INPUT, node, PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_MAP_BUFFERS,
                      params, 1);

    struct spa_source *timer = pw_loop_add_timer(pw_main_loop_get_loop(st.loop), check_parent, NULL);
    struct timespec second = {1, 0};
    pw_loop_update_timer(pw_main_loop_get_loop(st.loop), timer, &second, &second, false);

    pw_main_loop_run(st.loop);
    pw_stream_destroy(st.stream);
    pw_core_disconnect(core);
    pw_context_destroy(context);
    pw_main_loop_destroy(st.loop);
    pw_deinit();
    sd_bus_flush_close_unref(bus);
    return 0;
}
