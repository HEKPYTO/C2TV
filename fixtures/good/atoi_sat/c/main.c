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

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    uint32_t i = 0;
    int neg = 0;
    int64_t v = 0;
    int32_t r;
    *status = 0;
    *ret = 0;
    *outn = 0;
    (void)out;
    if (n > 0 && (in[0] == (uint8_t)'+' || in[0] == (uint8_t)'-')) {
        neg = in[0] == (uint8_t)'-';
        i = 1;
    }
    if (i >= n) { *status = 1; return; }
    for (; i < n; i++) {
        if (in[i] < (uint8_t)'0' || in[i] > (uint8_t)'9') { *status = 1; return; }
        v = v * 10 + (int64_t)(in[i] - (uint8_t)'0');
        if (v > 2147483648LL) v = 2147483648LL;
    }
    if (neg) {
        r = (v >= 2147483648LL) ? (-2147483647 - 1) : (int32_t)(-(int64_t)v);
    } else {
        r = (v > 2147483647LL) ? 2147483647 : (int32_t)v;
    }
    *ret = (uint64_t)(int64_t)r;
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
