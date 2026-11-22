#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define CAP 4096

static int read_all(uint8_t *dst, size_t n) {
    size_t got = 0;
    while (got < n) {
        size_t r = fread(dst + got, 1, n - got, stdin);
        if (r == 0) return 0;
        got += r;
    }
    return 1;
}

static uint32_t rd32(const uint8_t *p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static uint64_t rd64(const uint8_t *p) {
    return (uint64_t)rd32(p) | ((uint64_t)rd32(p + 4) << 32);
}

static void wr32(uint8_t *p, uint32_t v) {
    p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8);
    p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24);
}

static void wr64(uint8_t *p, uint64_t v) {
    wr32(p, (uint32_t)v); wr32(p + 4, (uint32_t)(v >> 32));
}

typedef struct {
    uint8_t buf[8];
    uint32_t cap;
    uint32_t head;
    uint32_t len;
} Ring;

static void ring_init(Ring *r, uint32_t cap) {
    uint32_t i;
    for (i = 0; i < 8; i++) r->buf[i] = 0;
    r->cap = cap;
    r->head = 0;
    r->len = 0;
}

static int ring_push(Ring *r, uint8_t v) {
    uint32_t idx;
    if (r->cap == 0 || r->len == r->cap) return 0;
    idx = (r->head + r->len) % r->cap;
    r->buf[idx] = v;
    r->len++;
    return 1;
}

static int ring_pop(Ring *r, uint8_t *v) {
    if (r->cap == 0 || r->len == 0) return 0;
    *v = r->buf[r->head];
    r->head = (r->head + 1) % r->cap;
    r->len--;
    return 1;
}

static uint32_t ring_contents(const Ring *r, uint8_t *out) {
    uint32_t i;
    for (i = 0; i < r->len; i++) {
        out[i] = r->buf[(r->head + i) % r->cap];
    }
    return r->len;
}

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    uint32_t cap;
    uint32_t i;
    Ring r;
    *status = 0;
    *ret = 0;
    *outn = 0;
    if (n < 4) {
        *status = 1;
        return;
    }
    cap = rd32(in);
    if (cap < 1 || cap > 8) {
        *status = 1;
        return;
    }
    ring_init(&r, cap);
    for (i = 4; i < n;) {
        uint8_t op = in[i++];
        if (op == 0) {
            if (i >= n || !ring_push(&r, in[i++])) {
                *status = 1;
                return;
            }
        } else if (op == 1) {
            uint8_t v;
            if (!ring_pop(&r, &v)) {
                *status = 1;
                return;
            }
        } else {
            *status = 1;
            return;
        }
    }
    *outn = ring_contents(&r, out);
    *ret = (uint64_t)r.len;
}

int main(void) {
    uint8_t hdr[4];
    uint8_t in[CAP];
    uint8_t out[CAP];
    uint8_t resp[20 + CAP];
    uint32_t n;
    uint64_t status = 0;
    uint64_t ret = 0;
    uint32_t outn = 0;
    if (!read_all(hdr, 4)) return 2;
    n = rd32(hdr);
    if (n > CAP) return 2;
    if (!read_all(in, n)) return 2;
    compute(in, n, &status, &ret, out, &outn);
    if (outn > CAP) return 2;
    wr64(resp, status);
    wr64(resp + 8, ret);
    wr32(resp + 16, outn);
    if (outn != 0) memcpy(resp + 20, out, outn);
    if (fwrite(resp, 1, 20 + outn, stdout) != (size_t)(20 + outn)) return 2;
    return 0;
}
