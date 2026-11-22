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

static void sift_down(uint32_t *h, uint32_t i, uint32_t n) {
    for (;;) {
        uint32_t l = 2u * i + 1u;
        uint32_t r = 2u * i + 2u;
        uint32_t s = i;
        uint32_t t;
        if (l < n && h[l] < h[s]) s = l;
        if (r < n && h[r] < h[s]) s = r;
        if (s == i) break;
        t = h[i];
        h[i] = h[s];
        h[s] = t;
        i = s;
    }
}

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    uint32_t cnt;
    uint32_t i;
    uint32_t a[8];
    *status = 0;
    *ret = 0;
    *outn = 0;
    if (n < 4u) {
        *status = 1;
        return;
    }
    cnt = rd32(in);
    if (cnt > 8u || n != 4u + 4u * cnt) {
        *status = 1;
        return;
    }
    for (i = 0; i < cnt; i++) {
        a[i] = rd32(in + 4u + 4u * i);
    }
    i = cnt / 2u;
    while (i > 0u) {
        i--;
        sift_down(a, i, cnt);
    }
    for (i = 0; i < cnt; i++) {
        wr32(out + 4u * i, a[i]);
    }
    *ret = (uint64_t)cnt;
    *outn = 4u * cnt;
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
